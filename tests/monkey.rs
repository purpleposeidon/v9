use v9::prelude::*;
use std::any::TypeId;

#[v9::table]
pub struct cheeses {
    pub flaming: bool,
}

#[test]
fn patch() {
    let mut universe = Universe::new();
    cheeses::Marker::register(&mut universe);
    type WeightCol = v9::column::Column<cheeses::Marker, f32>;
    universe.add_mut(
        TypeId::of::<WeightCol>(),
        WeightCol::new(),
    );
    universe.eval(|mut c: cheeses::Write| {
        c.push(cheeses::Row { flaming: true });
        c.push(cheeses::Row { flaming: false });
    });
}
