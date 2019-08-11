use v9::kernel::*;
use v9::prelude::*;
use v9::prelude_lib::*;
use std::ops::*;

// We'd like to be able to pass this scary thing with lifetimes as a KernelArg.
// However, that requires Any, which requires 'static, which this clearly is not.
#[derive(Debug)]
struct Scary<'a, 'b> {
    data: &'a mut Vec<&'b mut i32>,
}
impl<'a, 'b> Scary<'a, 'b> {
    // Well, we could FORCE IT to be static...
    // It works, but it's not very nice.
    unsafe fn forcecast(&mut self) -> &mut Scary<'static, 'static> {
        std::mem::transmute(self)
    }
}

// We can improve this by creating a wrapper!
// It'll extract the 'static, and expose it as an arbitrary lifetime.
struct Forcecast<'e, 'a, 'b> {
    data: &'e mut Scary<'a, 'b>,
}
impl<'e, 'a, 'b> Deref for Forcecast<'e, 'a, 'b> {
    type Target = Scary<'a, 'b>;
    fn deref(&self) -> &Scary<'a, 'b> { self.data }
}
impl<'e, 'a, 'b> DerefMut for Forcecast<'e, 'a, 'b> {
    fn deref_mut(&mut self) -> &mut Scary<'a, 'b> { self.data }
}
unsafe impl<'e, 'a, 'b> Extract for Forcecast<'e, 'a, 'b> {
    fn each_resource(_f: &mut dyn FnMut(TypeId, Access)) {}
    type Owned = Option<Forcecast<'static, 'static, 'static>>;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self::Owned {
        Some(Forcecast { data: rez.take_mut_downcast() })
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        let owned: Forcecast<'static, 'static, 'static> = (*owned).take().unwrap();
        // I'm not sure why the transmute is necessary?
        mem::transmute(owned)
    }
    type Cleanup = ();
}

#[test]
fn arg_passing_issue() {
    let universe = Universe::new();
    #[allow(unused_variables, unused_mut)]
    let mut leak: Option<&mut i32> = None;
    let mut k = Kernel::new(|s: KernelArg<&mut String>, b: Forcecast| {
        println!("{}", *s);
        println!("{:?}", *b);
        // Uncomment this code to verify that this scheme is (apparently) sound:
        //leak = Some(b.data.data[0]);
    });
    let mut val = format!("hello world!");
    k.push_arg_mut(&mut val);
    let mut n0 = 0;
    let mut n1 = 1;
    let mut data = vec![&mut n0, &mut n1];
    let data = &mut Scary { data: &mut data };
    k.push_arg_mut(unsafe { data.forcecast() });
    universe.run(&mut k);
}
