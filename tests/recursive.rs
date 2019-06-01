#[macro_use] extern crate v9;
use v9::prelude::*;


table! {
    pub struct person {
        pub id: u64,
        pub town: crate::town::Id,
        pub continent: crate::continent::Id,
    }
}

table! {
    pub struct town {
        pub id: u32,
        pub kingdom: crate::kingdom::Id,
    }
}

table! {
    pub struct kingdom {
        pub id: u16,
        pub continent: crate::continent::Id,
    }
}

table! {
    pub struct continent {
        pub id: u8,
    }
}

fn main() {
    let mut universe = Universe::new();
    person::Marker::register(&mut universe);
    town::Marker::register(&mut universe);
    kingdom::Marker::register(&mut universe);
    continent::Marker::register(&mut universe);
    universe.kmap(|mut p: person::Write, mut t: town::Write, mut k: kingdom::Write, mut c: continent::Write| {
        for id in 0..4 {
            let continent = c.push(continent::Row { id });
            for id in 0..4 {
                let kingdom = k.push(kingdom::Row { id, continent });
                for id in 0..4 {
                    let town = t.push(town::Row { id, kingdom });
                    for id in 0..4 {
                        p.push(person::Row { id, town, continent });
                    }
                }
            }
        }
    });
    universe.kmap(|p: person::Read| {
        for id in p.iter() {
            println!("{:?}", p.ref_row(id));
        }
    });
    println!();
    universe.kmap(|mut c: continent::Write| {
        c.remove(continent::FIRST);
    });
    universe.kmap(|p: person::Read| {
        for id in p.iter() {
            println!("{:?}", p.ref_row(id));
        }
    });
}
