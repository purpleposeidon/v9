#[macro_use] extern crate v9;
use v9::prelude::*;
use std::any::*;

decl_table! {
    pub struct person {
        pub id: u64,
        pub town: crate::town::Id,
        pub continent: crate::continent::Id,
    }
}

decl_table! {
    pub struct town {
        pub id: u32,
        pub kingdom: crate::kingdom::Id,
    }
}

decl_table! {
    pub struct kingdom {
        pub id: u16,
        pub continent: crate::continent::Id,
    }
}

decl_table! {
    pub struct continent {
        pub id: u8,
    }
}

#[test]
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
    println!();
    use v9::linkage::Select;
    use v9::id::RunList;
    use std::sync::*;
    let run0 = Arc::new(RwLock::new(RunList::default()));
    let run = run0.clone();
    universe.kmap(move |c: kingdom::Read| {
        let mut skip = true;
        for id in c.iter() {
            skip = !skip;
            if skip { continue; }
            use crate::v9::id::Check;
            let id = id.uncheck();
            run.write().unwrap().push(id);
        }
        let _ = dbg!(run.read().unwrap());
    });
    dbg!(TypeId::of::<continent::Marker>());
    dbg!(TypeId::of::<kingdom::Marker>());
    dbg!(TypeId::of::<town::Marker>());
    dbg!(TypeId::of::<person::Marker>());
    let run = Arc::try_unwrap(run0).unwrap().into_inner().unwrap();
    let mut ev = Select::from(run);
    universe.submit_event(&mut ev);
    println!("{:?}", ev.selection);
    println!("{:?}", ev.selection.get::<kingdom::Marker>().unwrap());
    println!("{:?}", ev.selection.get::<person::Marker>().unwrap());
}
