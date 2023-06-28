extern crate v9;

// Declare a couple tables.
#[v9::table]
#[row::derive(Copy)]
pub struct cheeses {
    pub quantity: f64,
    pub warehouse: crate::warehouses::Id,
    pub stinky: bool,
}

#[v9::table]
pub struct warehouses {
    pub coordinates: (i32, i32),
    pub on_fire: bool,
}

#[v9::property]
#[derive(Default, Debug)]
pub struct window {
    instance: u32,
}

fn main() {
    // We create a new Universe. We put everything we can in it!
    use v9::prelude::Universe;
    let mut universe = Universe::new();

    // But it doesn't know about the tables, so we must register them.
    use v9::prelude::Register;
    cheeses::Marker::register(&mut universe);
    warehouses::Marker::register(&mut universe);

    // Let's print out an inventory.
    use v9::kernel::Kernel;
    let mut dump_all = Kernel::new(|cheeses: cheeses::Read, warehouses: warehouses::Read| {
        println!("Warehouses:");
        for id in warehouses.iter() {
            println!("{:?} = {:?}", id, warehouses.ref_row(id));
        }
        println!("Cheeses:");
        for id in cheeses.iter() {
            println!("{:?} = {:?}", id, cheeses.ref_row(id));
        }
    });
    // The kernel holds our closure, and keeps track of all of the objects it requires.
    universe.run(&mut dump_all);
    // It's empty... we should add some things.
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
            let w3 = warehouses.push(warehouses::Row {
                coordinates: (8, 4),
                on_fire: false,
            });
            let mut quantity = 237.0;
            for wid in &[w0, w1, w2, w3] {
                cheeses.reserve(10);
                for _ in 0..10 {
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
    // v9 is somewhat low-level. Because a Kernel does allocation (and we HATE allocation)
    // you are expected to implement your own higher level manager.
    // We can use `universe.kmap` when we prefer to be sloppy.
    universe.run(&mut dump_all);
    // Kernels can also return values...
    let cheese_mass = universe.kmap_return(|cheeses: cheeses::Read| {
        let mut sum = 0.0;
        for id in cheeses.iter() {
            sum += cheeses.quantity[id];
        }
        sum
    });
    println!("Current cheese count: {:?}", cheese_mass);
    // Now we should see our data.
    // But remember how those warehouses were on fire?
    universe.kmap(
        |list: &mut warehouses::Ids, mut on_fire: warehouses::edit::on_fire| {
            let mut dousing = true;
            for wid in list.removing() {
                if on_fire[wid.id] {
                    if dousing {
                        dousing = false;
                        on_fire[wid.id] = false;
                    } else {
                        wid.remove();
                    }
                }
            }
        },
    );
    // v9 has ensured data consistency -- the cheese in the destroyed warehouses has been lost.
    universe.run(&mut dump_all);
    universe.kmap(|cheeses: cheeses::Read| {
        let mut n = 0;
        for _ in cheeses.iter() {
            n += 1;
        }
        assert_eq!(n, 30);
    });
}
