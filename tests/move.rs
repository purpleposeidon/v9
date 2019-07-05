use v9::prelude::*;

v9::decl_table! {
    #[raw_index(u64)]
    pub struct cheeses {
        pub quantity: f64,
        pub warehouse: crate::warehouses::Id,
        pub stinky: bool,
    }
}

v9::decl_table! {
    pub struct warehouses {
        pub coordinates: (i32, i32),
        pub on_fire: bool,
    }
}


#[test]
fn moving() {
    let universe = &mut Universe::new();
    cheeses::Marker::register(universe);
    warehouses::Marker::register(universe);

    universe.kmap(
        |mut warehouses: warehouses::Write, mut cheeses: cheeses::Write| {
            warehouses.reserve(3);
            let w0 = warehouses.push(warehouses::Row {
                coordinates: (0, 0),
                on_fire: true,
            });
            let w1 = warehouses.push(warehouses::Row {
                coordinates: (4, 9),
                on_fire: false,
            });
            let w2 = warehouses.push(warehouses::Row {
                coordinates: (8, 4),
                on_fire: true,
            });
            let mut quantity = 100.0;
            for wid in &[w0, w1, w2] {
                for _ in 0..3 {
                    cheeses.push(cheeses::Row {
                        quantity,
                        warehouse: *wid,
                        stinky: true,
                    });
                    quantity += 1.0;
                }
            }
        },
    );

    universe.kmap(|warehouses: warehouses::Read, cheeses: cheeses::Read| {
        for id in warehouses.iter() {
            println!("{:?} = {:?}", id, warehouses.ref_row(id));
        }
        for id in cheeses.iter() {
            println!("{:?} = {:?}", id, cheeses.ref_row(id));
        }
    });

    universe.kmap(
        |mut warehouses: warehouses::Write| {
            warehouses.remove(warehouses::Id::new(0));
        }
    );

    // FIXME: Y'know, we don't actually have a good way to move rows?
}
