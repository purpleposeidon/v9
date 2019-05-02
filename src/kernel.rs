use self::panic::AssertUnwindSafe;
use crate::prelude_lib::*;
use std::collections::HashSet;

impl Universe {
    pub fn run(&self, kernel: &mut Kernel) {
        unsafe {
            {
                let mut objects = self.objects.lock().unwrap();
                'again: loop {
                    let locks = &mut kernel.locks;
                    let vals = &mut kernel.vals;
                    locks.clear();
                    // `vals.clear()` goes below so that it can be used to pass in additional arguments.
                    for &(ty, acc) in &kernel.resources {
                        //[vals.len()..] {
                        let lock = objects
                            .get_mut(&ty)
                            .expect("unknown argument type in kernel");
                        if !lock.can(acc) {
                            objects = self.condvar.wait(objects).unwrap();
                            continue 'again;
                        }
                        locks.push((lock.deref_mut() as *mut Locked, acc));
                    }
                    for &mut (lock, acc) in locks {
                        let lock: &mut Locked = &mut *lock;
                        lock.acquire(acc);
                        let obj: *mut dyn Obj = lock.contents();
                        let obj: &mut dyn Obj = &mut *obj;
                        let obj: *mut dyn Obj = obj;
                        vals.push((obj, acc));
                    }
                    break;
                }
            }
            let ret = {
                let rez = Rez::new(mem::transmute(&kernel.vals[..]));
                let run = &mut kernel.run;
                let resources = &kernel.resources;
                panic::catch_unwind(AssertUnwindSafe(move || {
                    run(self, rez, &mut move || {
                        let mut objects = self.objects.lock().expect("unable to release locks");
                        for &(ty, acc) in resources {
                            let lock = objects.get_mut(&ty).expect("lost locked object");
                            lock.release(acc);
                        }
                    })
                }))
            };
            kernel.vals.clear();
            ret.unwrap_or_else(|e| panic::resume_unwind(e))
        }
    }

    /// Quick & dirty `Kernel` `run`ner. This is provided to simplify tests.
    pub fn kmap<Dump, K: KernelFn<Dump>>(&self, k: K) {
        self.run(&mut Kernel::new(k));
    }
}

/// Implemented for closures.
///
/// If your closure isn't a `Kernel`, ensure that:
/// 1. All arguments are `Extract`.
/// 2. There aren't too many
/// 3. (FIXME: Constraints on return value? Must be `()` for now.)
pub unsafe trait KernelFn<Dump>: 'static {
    fn each_resource(f: &mut dyn FnMut(TypeId, Access));

    unsafe fn run(&mut self, universe: &Universe, args: Rez, cleanup: &mut dyn FnMut());
}

pub struct Kernel {
    resources: Vec<(TypeId, Access)>,
    run: Box<dyn FnMut(&Universe, Rez, &mut dyn FnMut())>,
    locks: Vec<(*mut Locked, Access)>,
    vals: Vec<(*mut dyn Obj, Access)>,
}
impl Kernel {
    pub fn new<Dump, K: KernelFn<Dump>>(mut k: K) -> Self {
        let mut resources = vec![];
        let mut write = HashSet::new();
        let mut any = HashSet::new();
        K::each_resource(&mut |t, a| {
            resources.push((t, a));
            match a {
                Access::Read => {
                    if write.contains(&t) {
                        panic!("kernel has conflicting acquisitions on lock");
                    }
                }
                Access::Write => {
                    if any.contains(&t) {
                        panic!("kernel has conflicting acquisitions on lock");
                    }
                    write.insert(t);
                }
            }
            any.insert(t);
        });
        let locks = Vec::with_capacity(resources.len());
        let vals = Vec::with_capacity(resources.len());
        Kernel {
            resources,
            run: Box::new(move |universe, rez, cleanup| unsafe { k.run(universe, rez, cleanup) }),
            locks,
            vals,
        }
    }
    /// A kernel may have arguments that the `Universe` doesn't know about.
    /// Any such arguments must be at the front of the parameter list,
    /// and must be pushed in the correct order.
    pub fn push_arg(&mut self, obj: &dyn Obj) {
        self.vals
            .push((obj as *const dyn Obj as *mut dyn Obj, Access::Read));
    }
    pub fn push_arg_mut(&mut self, obj: &mut dyn Obj) {
        self.vals.push((obj as *mut dyn Obj, Access::Write));
    }
    pub fn clear_args(&mut self) {
        self.vals.clear();
    }
}

/// This wraps an argument to a kernel that does not exist in the `Universe`. It is provided using
/// `Kernel::push_arg`.
pub struct KernelArg<T> {
    val: T,
}
unsafe impl<'a, T: Obj> Extract for KernelArg<&'a T> {
    fn each_resource(_f: &mut dyn FnMut(TypeId, Access)) {}
    type Owned = &'a T;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self::Owned {
        rez.take_ref_downcast()
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        KernelArg { val: *owned }
    }
}
unsafe impl<'a, T: Obj> Extract for KernelArg<&'a mut T> {
    fn each_resource(_f: &mut dyn FnMut(TypeId, Access)) {}
    type Owned = &'a mut T;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self::Owned {
        rez.take_mut_downcast()
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        KernelArg { val: *owned }
    }
}
impl<T> Deref for KernelArg<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.val
    }
}
impl<T> DerefMut for KernelArg<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.val
    }
}

mod kernel_impl {
    #![allow(non_snake_case)]
    use super::*;
    macro_rules! impl_kernel {
        ($($A:ident),*) => {
            unsafe impl<$($A,)* X> KernelFn<($($A,)*)> for X
            where
                X: 'static,
                X: FnMut($($A),*),
                $($A: Extract,)*
            {
                fn each_resource(f: &mut dyn FnMut(TypeId, Access)) {
                    $(
                        $A::each_resource(f);
                    )*
                }
                unsafe fn run(&mut self, universe: &Universe, mut args: Rez, cleanup: &mut dyn FnMut()) {
                    $(let mut $A: $A::Owned = $A::extract(universe, &mut args);)*
                    {
                        $(let $A: $A = $A::convert(universe, &mut $A as *mut $A::Owned);)*
                        self($($A),*);
                    }
                    cleanup();
                    $($A::finish(universe, $A);)*
                }
            }
            impl_kernel! { @ $($A),* }
        };
        (@ $_:ident) => {};
        (@ $_:ident $(, $A:ident)*) => {
            // I wish we could pop the tail. 'A13, A14' is silly.
            impl_kernel! { $($A),* }
        };
    }
    impl_kernel! { A00, A01, A02, A03, A04, A05, A06, A07, A08, A09, A10, A11, A12, A13, A14 }
}
