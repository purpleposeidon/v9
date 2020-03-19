use v9::prelude::*;
use v9::event::Pushed;
use v9::kernel::KernelArg;
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
    universe.add_tracker_with_ref_arg::<_, _, Pushed<cheeses::Marker>>(|
        ev: KernelArg<&Pushed<cheeses::Marker>>,
        mut weights: v9::column::WriteColumn<cheeses::Marker, f32>, // direct access
    | {
        unsafe {
            let weights = weights.col.get_mut();
            for id in ev.ids.iter() {
                weights.data.push(100.1 * (id.0 as f32 + 1.0));
            }
        }
    });
    let id = universe.eval(|mut c: cheeses::Write| {
        c.push(cheeses::Row { flaming: true });
        c.push(cheeses::Row { flaming: false })
    });
    universe.eval(|weight: v9::column::ReadColumn<cheeses::Marker, f32>| {
        println!("{}", weight[id]);
    });
}
