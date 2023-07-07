//! Running functions over the Universe.

use crate::prelude_lib::*;
use std::borrow::Cow;
use std::collections::HashSet;
use std::cell::Cell;
use std::fmt;
use std::any::Any as StdAny;

fn describe_resources(resources: &[(Ty, Access)]) {
    if resources.is_empty() {
        eprintln!("\t\tKernel has no resources");
    } else {
        eprintln!("\t\tKernel uses {} resources:", resources.len());
    }
    for (ty, access) in resources {
        let a = match access {
            Access::Read  => "read  ",
            Access::Write => "write ",
        };
        let mut ty = format!("{:?}", ty);
        let pretty = &[
            // Stolen from ezty... hmm.
            ("alloc::boxed::", "Box"),
            ("alloc::collections::binary_heap::", "BinaryHeap"),
            ("alloc::collections::btree::map::", "BTreeMap"),
            ("alloc::collections::btree::set::", "BTreeSet"),
            ("alloc::collections::linked_list::", "LinkedList"),
            ("alloc::collections::vec_deque::", "VecDeque"),
            ("alloc::sync::", "Arc"),
            ("alloc::vec::", "Vec"),
            ("core::cell::", "Cell"),
            ("core::cell::", "RefCell"),
            ("core::option::", "Option"),
            ("core::result::", "Result"),
            ("std::collections::hash::map::", "HashMap"),
            ("std::collections::hash::set::", "HashSet"),
            ("std::sync::rwlock::", "RwLock"),
            // And more stuff
            ("v9::column::Column", "Column"),
            ("::in_v9::", "::"),
            ("::_v9_property_mod_", ""),
            ("::PropGeneric<", "<"),
            ("v9::id::IdList", "IdList"),
            // Just deal with it, I guess.
            ("triton::", ""),
            ("util::tagdb::Tag", "Tag"),
            ("alloc::string::String", "String"),
            ("lerp::Lerp", "Lerp"),
            ("nalgebra::base::dimension::", ""),
            ("space::rad::Rad", "Rad"),
            ("nalgebra::base::unit::Unit", "Unit"),
            ("Unit<nalgebra::geometry::quaternion::Quaternion<f32>>", "Quat"),
            ("v9::id::", ""),
            ("v9::column::Column", "Column"),
            ("::in_v9::", "::"),
            ("new_units::", ""),
            ("nalgebra::base::matrix::Matrix<f32, U3, U1, nalgebra::base::array_storage::ArrayStorage<f32, U3, U1>>", "V3"),
            ("_v9_property_mod_", ""),
            ("v9::event::", "v9:"),
            ("v9::linkage::", "v9:"),
            ("::PropGeneric", "="),
            ("core::option::Option", "Option"),
            ("core::result::Result", "Result"),
            ("triton::behaviors::QuatrexDefinition", "QuatrexDefinition"),
        ];
        for (ugly, pretty) in pretty {
            ty = ty.replace(ugly, pretty);
        }
        eprintln!("\t\t\t{} {}", a, ty);
    }
}

#[must_use]
pub struct ResetBuffer<'a> {
    pub(crate) universe: &'a Universe,
    pub name: &'a str,
    buffer: &'a mut LockBuffer,
}
impl Drop for ResetBuffer<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if self.name.is_empty() {
                eprintln!("NOTE: Panic in un-named kernel");
            } else {
                eprintln!("NOTE: Panic in kernel {}", self.name);
            }
            describe_resources(&self.buffer.resources);
            let mut objects = self.universe.objects.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            for &(ty, acc) in &self.buffer.resources {
                if let Some(obj) = objects.get_mut(&ty) {
                    // Sets poison as appropriate.
                    obj.release(acc);
                }
            }
            self.universe.condvar.notify_all();
        }
        self.buffer.vals.clear();
    }
}
impl<'a> ResetBuffer<'a> {
    fn done(self) {}
    pub fn cleanup(&self) -> PostCleanup {
        // The cleanup closure.
        // See comment in 'fn run' KernelFn impl.
        let mut objects = self.universe.objects.lock().expect("unable to release locks");
        for &(ty, acc) in &self.buffer.resources {
            let lock = objects.get_mut(&ty).expect("lost locked object");
            lock.release(acc);
        }
        self.universe.condvar.notify_all();
        PostCleanup { name: self.name, buffer: self.buffer }
    }
}
pub struct PostCleanup<'a> {
    pub name: &'a str,
    buffer: &'a LockBuffer,
}
impl Drop for PostCleanup<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if self.name.is_empty() {
                eprintln!("NOTE: Post-cleanup panic in un-named kernel");
            } else {
                eprintln!("NOTE: Post-cleanup panic in kernel {}", self.name);
            }
            describe_resources(&self.buffer.resources);
        }
    }
}

impl Universe {
    pub fn run(&self, kernel: &mut Kernel) {
        self.run_return::<()>(kernel)
    }
    pub fn run_return<Ret: StdAny>(&self, kernel: &mut Kernel) -> Ret {
        let mut ret: Option<Ret> = None;
        self.run_and_return_into(kernel, (&mut ret) as &mut dyn StdAny);
        ret.expect("return value not set")
    }
    unsafe fn prepare_buffer<'a>(&'a self, name: &'a str, buffer: &'a mut LockBuffer) -> ResetBuffer<'a> {
        let objects = self.objects.lock().expect("prepare_buffer locking objects failed");
        let _objects = self.condvar.wait_while(objects, |objects| {
            let locks = &mut buffer.locks;
            let resources = &mut buffer.resources;
            locks.clear();
            // `vals.clear()` goes below so that it can be used to pass in additional arguments.
            resources
                .iter()
                .enumerate()
                .any(|(argn, &(ty, acc))| {
                    let lock = objects
                        .get_mut(&ty)
                        .unwrap_or_else(|| {
                            panic!("kernel {:?} argument component {} (of {}) has unknown type {:?}", name, argn, resources.len(), ty)
                        });
                    if !lock.can(acc) {
                        true
                    } else {
                        locks.push((lock.deref_mut() as *mut Locked, acc));
                        false
                    }
                })
        }).expect("prepare_buffer condvar wait failed");
        for &mut (lock, acc) in &mut buffer.locks {
            let lock: &mut Locked = &mut *lock;
            lock.acquire(acc);
            let obj: *mut dyn AnyDebug = lock.contents();
            let obj: &mut dyn AnyDebug = &mut *obj;
            let obj: *mut dyn AnyDebug = obj;
            buffer.vals.push((obj, acc));
        }
        ResetBuffer {
            universe: self,
            name,
            buffer,
        }
    }
    unsafe fn execute_from_buffer<F>(
        &self,
        func: F,
        return_value: &mut dyn StdAny,
        cleanup: &mut ResetBuffer,
    )
    where
        F: FnOnce(Rez, &mut dyn StdAny, &mut ResetBuffer),
    {
        let rez = Rez::new(mem::transmute(&cleanup.buffer.vals[..]));
        func(rez, return_value, cleanup);
    }
    pub fn run_and_return_into(&self, kernel: &mut Kernel, return_value: &mut dyn StdAny) {
        // FIXME(soundness): Assert that all columns in a single table have same length.
        unsafe {
            let mut cleanup = self.prepare_buffer(&kernel.name, &mut kernel.buffer);
            self.execute_from_buffer(
                &mut kernel.run,
                return_value,
                &mut cleanup,
            );
            cleanup.done();
        }
    }
    pub fn eval<Dump, Ret, K>(&self, k: K) -> Ret
    where
        K: KernelFnOnce<Dump, Ret>,
    {
        // FIXME: There's some efficiency that could be squeezed outta this.
        // We could store a 'trusted kernel type', and skip the validation.
        let name = std::any::type_name::<K>();
        let ret = Cell::new(Option::<Ret>::None);
        let run = |rez: Rez, _ret: &mut dyn StdAny, cleanup: &mut ResetBuffer| {
            let got = unsafe { k.run(rez, cleanup) };
            ret.set(Some(got));
        };
        unsafe {
            let mut buffer = LockBuffer::new::<Dump, Ret, K>();
            let mut cleanup = self.prepare_buffer(name, &mut buffer);
            self.execute_from_buffer(
                run,
                &mut (),
                &mut cleanup,
            );
            cleanup.done();
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
///    - `kmap_return` and `run_return` requires `AnyDebug`, which means it must be `'static`.
pub unsafe trait KernelFn<Dump, Ret>: EachResource<Dump, Ret> {
    unsafe fn run(&mut self, args: Rez, cleanup: &ResetBuffer) -> Ret;
}

pub unsafe trait KernelFnOnce<Dump, Ret>: EachResource<Dump, Ret> {
    unsafe fn run(self, args: Rez, cleanup: &ResetBuffer) -> Ret;
}

pub unsafe trait EachResource<Dump, Ret> {
    // FIXME: It'd be nice to give a return value. However we can't because `Kernel` is dynamic.
    // FIXME: What if we passed in `&mut AnyDebug=Option<R>`?
    fn each_resource(f: &mut dyn FnMut(Ty, Access));
}

/// Works like a `Box<KernelFn>`.
#[must_use]
pub struct Kernel {
    run: Box<dyn FnMut(Rez, &mut dyn StdAny, &mut ResetBuffer) + 'static + Send + Sync>,
    buffer: LockBuffer,
    pub name: Cow<'static, str>,
}
struct LockBuffer {
    resources: Vec<(Ty, Access)>,
    locks: Vec<(*mut Locked, Access)>,
    vals: Vec<(*mut dyn AnyDebug, Access)>,
}
impl LockBuffer {
    fn new<Dump, Ret, K>() -> Self
    where
        K: EachResource<Dump, Ret>,
    {
        Self::new0(K::each_resource)
    }
    fn new0(each_resource: fn(&mut dyn FnMut(Ty, Access))) -> Self {
        let mut resources = vec![];
        let mut write = HashSet::new();
        let mut any = HashSet::new();
        each_resource(&mut |t, a| {
            resources.push((t, a));
            match a {
                Access::Read => {
                    if write.contains(&t) {
                        panic!("kernel has conflicting acquisitions on lock: {:?}", t);
                    }
                }
                Access::Write => {
                    if any.contains(&t) {
                        panic!("kernel has conflicting acquisitions on lock: {:?}", t);
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
}

impl fmt::Debug for Kernel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.name.is_empty() {
            write!(f, "<anonymous kernel>")
        } else {
            write!(f, "kernel {}", self.name)
        }
    }
}

#[no_mangle]
fn v9_before_kernel_run() {}

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
            run: Box::new(move |rez, ret, cleanup| unsafe {
                let ret: &mut Option<Ret> = ret.downcast_mut().expect("return type mismatch");
                v9_before_kernel_run();
                *ret = Some(k.run(rez, cleanup));
            }),
            buffer: LockBuffer::new::<Dump, Ret, K>(),
            name: std::any::type_name::<K>().into(),
        }
    }
    /// A kernel may have arguments that the `Universe` doesn't know about.
    /// Any such arguments must be at the front of the parameter list,
    /// and must be pushed in the same order as the parameters.
    /// The parameters themselves must be wrapped in `KernelArg<&T>`.
    /// So, the kernel's parameters must be `|t: KernelArg<&T>, m: KernelArg<&mut M>, ...|`,
    /// and the kernel is called like this
    ///
    /// ```no_compile
    /// kernel
    ///     .with_args()
    ///     .arg(&t)
    ///     .arg_mut(&mut m)
    ///     .run(universe);
    /// ```
    pub fn with_args(&mut self) -> PushArgs {
        PushArgs(Some(self))
    }
    pub fn resources(&self) -> &[(Ty, Access)] { &self.buffer.resources }
}
pub struct PushArgs<'a>(Option<&'a mut Kernel>);
impl<'a> PushArgs<'a> {
    fn push(&mut self, obj: *mut dyn AnyDebug, access: Access) {
        self.0.as_mut().unwrap().buffer.vals.push((obj, access));
    }
    pub fn arg<'b>(mut self, obj: &'b dyn AnyDebug) -> PushArgs<'b>
    where
        'a: 'b,
    {
        let obj = obj as *const dyn AnyDebug as *mut dyn AnyDebug;
        self.push(obj, Access::Read);
        self
    }
    pub fn arg_mut<'b>(mut self, obj: &'b mut dyn AnyDebug) -> PushArgs<'b>
    where
        'a: 'b,
    {
        let obj = obj as *mut dyn AnyDebug;
        self.push(obj, Access::Write);
        self
    }
    pub fn run(mut self, universe: &Universe) {
        let k = self.0.take().unwrap();
        universe.run(k)
    }
    pub fn run_return<Ret: StdAny>(mut self, universe: &Universe) -> Ret {
        let k = self.0.take().unwrap();
        universe.run_return::<Ret>(k)
    }
}
impl<'a> Drop for PushArgs<'a> {
    fn drop(&mut self) {
        if let Some(k) = self.0.take() {
            k.buffer.vals.clear();
        }
    }
}

/// This wraps an argument to a kernel that does not exist in the `Universe`. It is provided using
/// `Kernel::with_args()`.
///
/// It's much nicer to have the thing live in the `Universe`,
/// but sometimes a kernel requires a non-`'static` argument.
// FIXME: Uhm, sure that's nice and all, but it still *ACTUALLY* requres 'static.
pub struct KernelArg<T> {
    val: T,
}
unsafe impl<'a, T: AnyDebug> Extract for KernelArg<&'a T> {
    fn each_resource(_f: &mut dyn FnMut(Ty, Access)) {}
    type Owned = &'a T;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self::Owned {
        rez.take_ref_downcast()
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        KernelArg { val: *owned }
    }
    type Cleanup = ();
}
unsafe impl<'a, T: AnyDebug> Extract for KernelArg<&'a mut T> {
    fn each_resource(_f: &mut dyn FnMut(Ty, Access)) {}
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
            fn each_resource(f: &mut dyn FnMut(Ty, Access)) {
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
            unsafe fn run(&mut self, mut args: Rez, cleanup: &ResetBuffer) -> Ret {
                $(let mut $A: $A::Owned = $A::extract(cleanup.universe, &mut args);)*
                let ret = {
                    $(let $A: $A = $A::convert(cleanup.universe, &mut $A as *mut $A::Owned);)*
                    self($($A),*)
                };
                $(let $A: $A::Cleanup = $A::Cleanup::pre_cleanup($A, cleanup.universe);)*
                let _post_cleanup = cleanup.cleanup(); // Releases the locks.
                $($A.post_cleanup(cleanup.universe);)*
                ret
            }
        }
        #[allow(non_snake_case)]
        unsafe impl<$($A,)* Ret, X> KernelFnOnce<($($A,)*), Ret> for X
        where
            X: FnOnce($($A),*) -> Ret,
            $($A: Extract,)*
        {
            unsafe fn run(self, mut args: Rez, cleanup: &ResetBuffer) -> Ret {
                $(let mut $A: $A::Owned = $A::extract(cleanup.universe, &mut args);)*
                let ret = {
                    $(let $A: $A = $A::convert(cleanup.universe, &mut $A as *mut $A::Owned);)*
                    self($($A),*)
                };
                $(let $A: $A::Cleanup = $A::Cleanup::pre_cleanup($A, cleanup.universe);)*
                let _post_cleanup = cleanup.cleanup(); // Releases the locks.
                $($A.post_cleanup(cleanup.universe);)*
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
    fn each_resource(_f: &mut dyn FnMut(Ty, Access)) {}
}
unsafe impl<X, Ret> KernelFn<(), Ret> for X
where
    X: FnMut() -> Ret,
{
    unsafe fn run(&mut self, _args: Rez, cleanup: &ResetBuffer) -> Ret {
        let ret = self();
        cleanup.cleanup();
        ret
    }
}

/// ```compile_fail
/// #[v9::table]
/// pub struct pets {
///     pub name: String,
///     pub hungry: bool,
/// }
/// fn shouldnt_compile() {
///     let u = v9::prelude::Universe::new();
///     let mut some_ref = None;
///     u.eval(move |pets: pets::Read| {
///         for id in pets.iter() {
///             some_ref = Some(&pets.name[id]);
///         }
///     });
/// }
/// ```
#[cfg(doctest)]
struct UnsafetyTest;
