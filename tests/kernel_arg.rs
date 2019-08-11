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

unsafe impl<'e, 'a, 'b> Extract for &'e mut Scary<'a, 'b> {
    fn each_resource(_f: &mut dyn FnMut(TypeId, Access)) {}
    type Owned = &'e mut Scary<'static, 'static>;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self::Owned {
        rez.take_mut_downcast()
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        // FIXME: How sound is this?
        // I'm primarily worried about multiple mutable aliasing.
        // It counts as being borrowed...right?
        let owned: &mut Scary<'static, 'static> = *owned;
        // What's up with this transmute?
        // &'static A -> &'a A is fine.
        // There's some implicit bounds being put on Scary.
        mem::transmute(owned)
    }
    type Cleanup = ();
}

// ...Okay, but there's still problems here! :|
// You can extract Scary<'static, 'static>.

#[test]
fn arg_passing_issue() {
    let universe = Universe::new();
    #[allow(unused_variables, unused_mut)]
    let mut leak: Option<&mut i32> = None;
    let mut k = Kernel::new(|s: KernelArg<&mut String>, b: &mut Scary| {
        println!("{}", *s);
        println!("{:?}", *b);
        // Uncomment this code to verify that this scheme is (apparently) sound:
        //leak = Some(b.data[0]);
    });
    let mut val = format!("hello world!");
    k.push_arg_mut(&mut val);
    let mut n0 = 0;
    let mut n1 = 1;
    let mut data = vec![&mut n0, &mut n1];
    let mut data = Scary { data: &mut data };
    k.push_arg_mut(unsafe { data.forcecast() });
    universe.run(&mut k);
}
