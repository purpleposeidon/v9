use v9::object::Register;

v9::decl_table! {
    pub struct cheese {
        pub stank: u64,
        pub temperature: f64,
    }
}

#[test]
fn doesnt_ice() {
    let mut universe = v9::object::Universe::new();
    cheese::Marker::register(&mut universe);
    universe.eval(|_stank: cheese::read::stank| {
    });
}
