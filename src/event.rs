//! Mechanisms for responding to events.

use crate::column::Column;
use crate::prelude_lib::*;

/// Event handlers for an event `E`.
// FIXME: Events should use RunIter.
pub struct Tracker<E> {
    pub handlers: Vec<Box<FnMut(&Universe, &E)>>,
}
impl<E: 'static> Obj for Tracker<E> {}
impl Universe {
    pub fn submit_event<E: 'static>(&self, e: &E) {
        let ty = &mut TypeId::of::<Tracker<E>>();
        unsafe {
            let event = {
                let mut objects = self.objects.write().unwrap();
                if let Some(locked) = objects.get_mut(ty) {
                    locked.acquire(Access::Write);
                    let obj: &mut dyn Obj = &mut *locked.contents();
                    obj.downcast_mut::<Tracker<E>>().unwrap()
                } else {
                    panic!("an event should not be created if there are no handlers");
                }
            };
            for handler in &mut event.handlers {
                handler(self, e);
            }
            if (cfg!(debug) || cfg!(test)) && event.handlers.is_empty() {
                panic!("if all handlers are removed from a tracker, it should be removed");
            }
        }
        let mut objects = self.objects.write().unwrap();
        objects
            .get_mut(ty)
            .expect("lost locked object")
            .release(Access::Write);
    }
    pub fn add_tracker<E: 'static>(&self, f: Box<FnMut(&Universe, &E)>) {
        let ty = TypeId::of::<Tracker<E>>();
        let mut objects = self.objects.write().unwrap();
        let obj = objects
            .entry(ty)
            .or_insert_with(|| Locked::new(Box::new(Tracker::<E> { handlers: vec![] })));
        obj.acquire(Access::Write);
        unsafe {
            let obj: &mut dyn Obj = &mut *obj.contents();
            let obj: &mut Tracker<E> = obj.downcast_mut().unwrap();
            obj.handlers.push(f);
        }
        obj.release(Access::Write);
    }
}

#[cfg(test)]
mod test_tracking {
    use super::*;

    table! {
        pub struct ships {
            pub name: Name,
            pub weight: u32,
        }
    }

    table! {
        pub struct sailors {
            pub name: Name,
            pub ship: ships::RowId,
        }
    }

    #[test]
    fn two_tables() {
        let universe = &mut Universe::new();
        ships::Marker::register(universe);
        sailors::Marker::register(universe);
    }

    #[test]
    fn basics() {
        let universe = &mut Universe::new();
        ships::Marker::register(universe);
        sailors::Marker::register(universe);
        println!("hello there");
        universe.kmap(|mut ships: ships::Write, mut sailors: sailors::Write| {
            let titanic = ships.push(ships::Row {
                name: "RMS Titanic",
                weight: 10,
            });
            let boaty_mcboatface = ships.push(ships::Row {
                name: "Boaty McBoatface",
                weight: 20,
            });
            let lusitania = ships.push(ships::Row {
                name: "RMS Lusitania",
                weight: 30,
            });
            let _mont_blanc = ships.push(ships::Row {
                name: "SS Mont-Blanc",
                weight: 40,
            });

            sailors.push(sailors::Row {
                ship: titanic,
                name: "Alice",
            });
            sailors.push(sailors::Row {
                ship: titanic,
                name: "Bob",
            });
            sailors.push(sailors::Row {
                ship: boaty_mcboatface,
                name: "Charles",
            });
            sailors.push(sailors::Row {
                ship: boaty_mcboatface,
                name: "Darude",
            });
            sailors.push(sailors::Row {
                ship: titanic,
                name: "Eve",
            });
            sailors.push(sailors::Row {
                ship: lusitania,
                name: "Frank",
            });
        });
        universe.kmap(|ships: ships::Read, sailors: sailors::Read| {
            println!("\nShips:");
            for id in ships.iter() {
                let ship = ships.ref_row(id);
                println!("{:?} = {:?}", id, ship);
            }
            println!("\nSailors:");
            for id in sailors.iter() {
                let sailor = sailors.ref_row(id);
                println!("{:?} = {:?}", id, sailor);
            }
        });
        println!("\nDeleting...");
        universe.kmap(
            |ships: ships::List, names: ships::read::name, weight: ships::read::weight| {
                let mut sunk = false;
                for f in ships.removing(&names) {
                    if weight[f] == 20 {
                        println!("The {} is sinking! Oh, the humanity!", names[f]);
                        f.remove();
                        assert!(!sunk);
                        sunk = true;
                    }
                }
                assert!(sunk);
            },
        );
        universe.kmap(|ships: ships::Read, sailors: sailors::Read| {
            println!("\nAll Ships:");
            let mut count = 0;
            for id in ships.iter() {
                let ship = ships.ref_row(id);
                println!("{:?} = {:?}", id, ship);
                assert!(!ship.name.contains("Boaty"));
                count += 1;
            }
            assert_eq!(count, 3);
            println!("\nSailors:");
            for id in sailors.iter_all() {
                let sailor = sailors.ref_row(id);
                println!("{:?} = {:?}", id, sailor);
            }
            println!();
            let mut count = 0;
            for id in sailors.iter() {
                let sailor = sailors.ref_row(id);
                println!("{:?} = {:?}", id, sailor);
                assert_ne!(sailor.name, &"Darude");
                assert_ne!(sailor.name, &"Charles");
                count += 1;
            }
            assert_eq!(count, 6 - 2);
        });
    }
}

// FIXME: Rename to `Push, Edit, Move, Delete` ?
pub struct Pushed<M: TableMarker> {
    pub range: UncheckedIdRange<M>,
}
impl<M: TableMarker> Obj for Pushed<M> {}
pub struct Edited<M: TableMarker, T: 'static> {
    pub(crate) col: &'static Column<M, T>,
    pub new: Vec<(Id<M>, T)>,
}
impl<M: TableMarker, T: 'static> Obj for Edited<M, T> {}
impl<M: TableMarker, T> Edited<M, T> {
    pub fn col<'a>(&'a self) -> &'a Column<M, T> {
        self.col
    }
}

pub struct Moved<M: TableMarker> {
    /// (old, new)
    pub ids: Vec<(Id<M>, Id<M>)>,
}
impl<M: TableMarker> Obj for Moved<M> {}
pub struct Deleted<M: TableMarker> {
    pub ids: Vec<Id<M>>,
}
impl<M: TableMarker> Obj for Deleted<M> {}
