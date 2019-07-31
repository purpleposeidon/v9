//! Running functions over the Universe.

use crate::prelude_lib::*;
use self::panic::AssertUnwindSafe;
use std::collections::HashSet;
use std::cell::Cell;

impl Universe {
    pub fn run(&self, kernel: &mut Kernel) {
        self.run_return::<()>(kernel)
    }
    pub fn run_return<Ret: StdAny>(&self, kernel: &mut Kernel) -> Ret {
        let mut ret: Option<Ret> = None;
        self.run_and_return_into(kernel, (&mut ret) as &mut StdAny);
        ret.expect("return value not set")
    }
    pub unsafe fn prepare_buffer(&self, buffer: &mut LockBuffer) {
        'again: loop {
            let mut objects = self.objects.write().unwrap();
            let locks = &mut buffer.locks;
            let vals = &mut buffer.vals;
            let resources = &buffer.resources;
            locks.clear();
            // `vals.clear()` goes below so that it can be used to pass in additional arguments.
            for (argn, &(ty, acc)) in resources.iter().enumerate() {
                let lock = objects
                    .get_mut(&ty)
                    .unwrap_or_else(|| {
                        panic!("kernel argument component {} (of {}) has unknown type {:?}", argn, resources.iter().count(), ty)
                    });
                if !lock.can(acc) {
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
    pub unsafe fn execute_from_buffer<F>(&self, buffer: &mut LockBuffer, func: F, return_value: &mut StdAny)
    where
        F: FnOnce(&Universe, Rez, &mut StdAny, &mut dyn FnMut()),
    {
        let rez = Rez::new(mem::transmute(&buffer.vals[..]));
        let resources = &buffer.resources;
        let unwind = panic::catch_unwind(AssertUnwindSafe(move || {
            func(self, rez, return_value, &mut move || {
                // The cleanup closure.
                // See comment in 'fn run' KernelFn impl.
                let mut objects = self.objects.write().expect("unable to release locks");
                for &(ty, acc) in resources {
                    let lock = objects.get_mut(&ty).expect("lost locked object");
                    lock.release(acc);
                }
            });
        }));
        buffer.vals.clear();
        unwind.unwrap_or_else(|e| panic::resume_unwind(e));
    }
    pub fn run_and_return_into(&self, kernel: &mut Kernel, return_value: &mut StdAny) {
        // FIXME(soundness): Assert that all columns in a single table have same length.
        unsafe {
            self.prepare_buffer(&mut kernel.buffer);
            self.execute_from_buffer(&mut kernel.buffer, &mut kernel.run, return_value);
        }
    }
    pub fn eval<Dump, Ret, K>(&self, k: K) -> Ret
    where
        K: KernelFnOnce<Dump, Ret>,
    {
        // FIXME: There's some efficiency that could be squeezed outta this.
        // We could store a 'trusted kernel type', and skip the validation.
        unsafe {
            let mut buffer = LockBuffer::new::<Dump, Ret, K>();
            self.prepare_buffer(&mut buffer);
            let ret = Cell::new(Option::<Ret>::None);
            let run = |universe: &Universe, rez: Rez, _ret: &mut StdAny, cleanup: &mut dyn FnMut()| {
                let got = k.run(universe, rez, cleanup);
                ret.set(Some(got));
            };
            self.execute_from_buffer(&mut buffer, run, &mut ());
            ret.into_inner().take().expect("return value not set")
        }
    }

    /// Quick & dirty `Kernel` `run`ner. This is provided to simplify tests.
    // FIXME: Delete this.
    pub fn kmap<Dump, K>(&self, k: K)
    where
        K: KernelFn<Dump, ()>,
        K: 'static + Send + Sync,
        Dump: Send + Sync,
    {
        self.kmap_return::<(), _, _>(k)
    }
    pub fn kmap_return<Ret, Dump, K>(&self, k: K) -> Ret
    where
        Ret: StdAny,
        K: KernelFn<Dump, Ret>,
        K: 'static + Send + Sync,
        Dump: Send + Sync,
    {
        self.run_return::<Ret>(&mut Kernel::new(k))
    }
}

/// Implemented for certain closures.
///
/// If your closure isn't a `Kernel`, ensure that:
/// 1. All arguments are `Extract`. (You can test this by writing `fn assert<T: Extract>() {}
///    assert::<T>();`)
/// 2. You don't have an unreasonable number of arguments. (If necessary, you can group them up via `decl_context!`.)
/// 3. The return value is appropriate. `Kernel` itself has no restrictions on the return type,
///    however:
///    - `kmap` requires the return value be `()`.
///    - `kmap_return` and `run_return` requires `Any`, which means it must be `'static`.
pub unsafe trait KernelFn<Dump, Ret>: EachResource<Dump, Ret> {
    unsafe fn run(&mut self, universe: &Universe, args: Rez, cleanup: &mut dyn FnMut()) -> Ret;
}

pub unsafe trait KernelFnOnce<Dump, Ret>: EachResource<Dump, Ret> {
    unsafe fn run(self, universe: &Universe, args: Rez, cleanup: &mut dyn FnMut()) -> Ret;
}

pub unsafe trait EachResource<Dump, Ret> {
    // FIXME: It'd be nice to give a return value. However we can't because `Kernel` is dynamic.
    // FIXME: What if we passed in `&mut Any=Option<R>`?
    fn each_resource(f: &mut dyn FnMut(TypeId, Access));
}

/// Works like a `Box<KernelFn>`.
pub struct Kernel {
    run: Box<dyn FnMut(&Universe, Rez, &mut StdAny, &mut dyn FnMut()) + 'static + Send + Sync>,
    buffer: LockBuffer,
}
pub struct LockBuffer {
    resources: Vec<(TypeId, Access)>,
    locks: Vec<(*mut Locked, Access)>,
    vals: Vec<(*mut dyn Obj, Access)>,
}
impl LockBuffer {
    pub fn new<Dump, Ret, K>() -> Self
    where
        K: EachResource<Dump, Ret>,
    {
        Self::new0(K::each_resource)
    }
    fn new0(each_resource: fn(&mut dyn FnMut(TypeId, Access))) -> Self {
        let mut resources = vec![];
        let mut write = HashSet::new();
        let mut any = HashSet::new();
        each_resource(&mut |t, a| {
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
        LockBuffer { resources, locks, vals }
    }
    pub fn new_for<Dump, Ret, K>(_func: &K) -> Self
    where
        K: KernelFn<Dump, Ret>,
        K: 'static + Send + Sync,
    {
        Self::new::<Dump, Ret, K>()
    }
}
// This seems janky, but I think it's barely sound?
// locks, vals: Only modified through a &mut reference.
// run: Well, I've put Send+Sync bounds on everything.
unsafe impl Send for LockBuffer {}
unsafe impl Sync for LockBuffer {}
impl Kernel {
    pub fn new<Dump, Ret, K>(mut k: K) -> Self
    where
        Ret: StdAny,
        K: KernelFn<Dump, Ret>,
        K: 'static + Send + Sync,
        Dump: Send + Sync,
    {
        Kernel {
            // Strange that we must duplicate this...
            run: Box::new(move |universe, rez, ret, cleanup| unsafe {
                let ret: &mut Option<Ret> = ret.downcast_mut().expect("return type mismatch");
                *ret = Some(k.run(universe, rez, cleanup));
            }),
            buffer: LockBuffer::new::<Dump, Ret, K>(),
        }
    }
    /// A kernel may have arguments that the `Universe` doesn't know about.
    /// Any such arguments must be at the front of the parameter list,
    /// and must be pushed in the correct order.
    pub fn push_arg(&mut self, obj: &dyn Obj) {
        let obj = obj as *const dyn Obj as *mut dyn Obj;
        self.buffer.vals.push((obj, Access::Read));
    }
    pub fn push_arg_mut(&mut self, obj: &mut dyn Obj) {
        let obj = obj as *mut dyn Obj;
        self.buffer.vals.push((obj, Access::Write));
    }
    pub fn clear_args(&mut self) {
        self.buffer.vals.clear();
    }
}

/// This wraps an argument to a kernel that does not exist in the `Universe`. It is provided using
/// `Kernel::push_arg` before running the kernel.
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
    type Cleanup = ();
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
    type Cleanup = ();
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

macro_rules! impl_kernel {
    ($($A:ident),*) => {
        unsafe impl<$($A,)* Ret, X> EachResource<($($A,)*), Ret> for X
        where
            X: FnOnce($($A),*) -> Ret,
            $($A: Extract,)*
        {
            fn each_resource(f: &mut dyn FnMut(TypeId, Access)) {
                $(
                    $A::each_resource(f);
                )*
            }
        }
        #[allow(non_snake_case)]
        unsafe impl<$($A,)* Ret, X> KernelFn<($($A,)*), Ret> for X
        where
            X: FnMut($($A),*) -> Ret,
            $($A: Extract,)*
        {
            unsafe fn run(&mut self, universe: &Universe, mut args: Rez, cleanup: &mut dyn FnMut()) -> Ret {
                $(let mut $A: $A::Owned = $A::extract(universe, &mut args);)*
                let ret = {
                    $(let $A: $A = $A::convert(universe, &mut $A as *mut $A::Owned);)*
                    self($($A),*)
                };
                $(let $A: $A::Cleanup = $A::Cleanup::pre_cleanup($A, universe);)*
                cleanup(); // Releases the locks.
                $($A.post_cleanup(universe);)*
                ret
            }
        }
        #[allow(non_snake_case)]
        unsafe impl<$($A,)* Ret, X> KernelFnOnce<($($A,)*), Ret> for X
        where
            X: FnOnce($($A),*) -> Ret,
            $($A: Extract,)*
        {
            unsafe fn run(self, universe: &Universe, mut args: Rez, cleanup: &mut dyn FnMut()) -> Ret {
                $(let mut $A: $A::Owned = $A::extract(universe, &mut args);)*
                let ret = {
                    $(let $A: $A = $A::convert(universe, &mut $A as *mut $A::Owned);)*
                    self($($A),*)
                };
                $(let $A: $A::Cleanup = $A::Cleanup::pre_cleanup($A, universe);)*
                cleanup(); // Releases the locks.
                $($A.post_cleanup(universe);)*
                ret
            }
        }
        impl_kernel! { @ $($A),* }
    };
    (@ $_:ident) => {};
    (@ $_:ident $(, $A:ident)*) => {
        impl_kernel! { $($A),* }
    };
}
impl_kernel! { A14, A13, A12, A11, A10, A09, A08, A07, A06, A05, A04, A03, A02, A01, A00 }
unsafe impl<X, Ret> EachResource<(), Ret> for X
where
    X: FnMut() -> Ret,
{
    fn each_resource(_f: &mut dyn FnMut(TypeId, Access)) {}
}
unsafe impl<X, Ret> KernelFn<(), Ret> for X
where
    X: FnMut() -> Ret,
{
    unsafe fn run(&mut self, _universe: &Universe, _args: Rez, cleanup: &mut dyn FnMut()) -> Ret {
        let ret = self();
        cleanup();
        ret
    }
}
