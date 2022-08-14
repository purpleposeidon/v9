use v9::prelude::*;

v9::decl_property! { THING: ~bool }

use std::panic::{self, AssertUnwindSafe};

#[test]
#[ignore]
fn main() {
    let mut u = Universe::new();
    THING::register(&mut u);
    let r = panic::catch_unwind(AssertUnwindSafe(|| {
        u.eval(|_thing: &mut THING| {
            panic!("*gasp!* He's been poisoned!");
        });
    }));
    //println!("{:?}", r);
    assert!(r.is_err());
    let r = panic::catch_unwind(AssertUnwindSafe(|| {
        u.eval(|_thing: &mut THING| {
            panic!("expected poison");
        });
    }));
    //println!("{:?}", r);
    assert!(r.is_err());
    println!("I'm fine.");
}
