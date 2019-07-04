#[macro_use] extern crate v9;


pub struct Meh {
    val: i32,
}

property! { MY_PROPERTY: Meh = Meh { val: 42 }; }


property! { ASSERT_DOESNT_COMPILE_HAS_NICE_ERROR: ~i32 }

table! {
    pub struct boop {
        pub foo: bool,
    }
}

context! {
    struct Stuff {
        //test: str,
        pub booper: boop::Read,
        pub the_property: &mut MY_PROPERTY,
    }
}

fn main() {
    use v9::prelude_lib::*;
    let mut universe = Universe::new();
    MY_PROPERTY::register(&mut universe);
    boop::Marker::register(&mut universe);
    universe.kmap(|stuff: Stuff| {
        stuff.the_property.val += 10;
    });
    universe.kmap(|stuff: Stuff| {
        assert_eq!(stuff.the_property.val, 52);
    });
}
