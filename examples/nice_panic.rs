extern crate v9;
use v9::prelude::*;
use v9::kernel::Kernel;

// Declare a couple tables.
#[v9::table]
pub struct cheeses {
    pub quantity: f64,
}

fn main() {
    let universe = &mut Universe::new();
    cheeses::Marker::register(universe);
    let k = |_cheeses: cheeses::Read| {
        panic!("\"Panic? In MY disco?\" It's more likely that you'd think!");
    };
    let mut k: Kernel = Kernel::new(k);
    k.name = "/examples/nice_panic.rs::disco".into();
    universe.run(&mut k);
}
