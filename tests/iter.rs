v9::decl_table! {
    pub struct my_table {
        pub names: String,
        pub age: f64,
    }
}

use v9::prelude::*;

#[test]
fn main() {
    let universe = &mut Universe::new();
    my_table::Marker::register(universe);
    universe.kmap(|names: my_table::read::names, ids: &my_table::Ids| {
        for id in ids.iter() {
            println!("{:?}", names[id]);
        }
    });
}
