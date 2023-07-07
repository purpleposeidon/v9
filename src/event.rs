//! Mechanisms for responding to events. No more than one event of each type should be emitted at a
//! time; instead we coalesce bulk changes under a single event.

use crate::column::Column;
use crate::prelude_lib::*;
use std::fmt;
use ezty::type_name;

pub type Handler<E> = Box<dyn FnMut(&Universe, &mut E) + Send + Sync>;

/// Event handlers for an event `E`.
// FIXME: Events should use RunIter.
#[derive(Default)]
pub struct Tracker<E: 'static + Send + Sync> {
    handlers: Vec<Handler<E>>,
}
impl<E: 'static + Send + Sync> fmt::Debug for Tracker<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Tracker<{}>({} handlers)", type_name::<E>(), self.handlers.len())
    }
}
impl<E: 'static + Send + Sync> Tracker<E> {
    pub fn new() -> Self {
        Tracker {
            handlers: vec![],
        }
    }
}
impl Universe {
    pub fn submit_event<E: AnyDebug + Send + Sync>(&self, e: &mut E) {
        let ty = &Ty::of::<Tracker<E>>();
        self.submit_event0(ty, &mut |event: &mut dyn AnyDebug| {
            let event = event.downcast_mut::<Tracker<E>>().unwrap();
            for handler in &mut event.handlers {
                handler(self, e);
            }
            if (cfg!(debug) || cfg!(test)) && event.handlers.is_empty() {
                panic!("if all handlers are removed from a tracker, it should be removed: {:?}", ty);
            }
        });
    }
    fn submit_event0(&self, ty: &Ty, then: &mut dyn FnMut(&mut dyn AnyDebug)) {
        let event = unsafe {
            let mut objects = self.objects.lock().unwrap();
            if let Some(locked) = objects.get_mut(ty) {
                locked.acquire(Access::Write);
                let obj: &mut dyn AnyDebug = &mut *locked.contents();
                obj
            } else {
                // FIXME: Yeah, idk, it's not great, but also, like, whatever. v9 is now creating
                // these events regardless of if there's a tracker because for its own events there
                // are in fact always trackers from IdList.
                //panic!("an event should not be created if there are no handlers: {:?}", type_name::<E>());
                return;
            }
        };
        let _release = crate::util::Defer(|| {
            let mut objects = self.objects.lock().unwrap();
            objects
                .get_mut(ty)
                .expect("lost locked object")
                .release(Access::Write);
        });
        then(event);
    }
    pub fn is_tracked<E: 'static + Send + Sync>(&self) -> bool {
        self.has_ty(Ty::of::<Tracker<E>>())
    }
    /// `owner` should be `Ty::of::<LocalTableMarker>()`.
    pub fn add_tracker<E: 'static + Send + Sync, F: FnMut(&Universe, &mut E) + 'static + Send + Sync>(&self, f: F) {
        self.add_tracker_box(Box::new(f))
    }
    fn add_tracker_box<E: 'static + Send + Sync>(&self, f: Box<dyn FnMut(&Universe, &mut E) + Send + Sync>) {
        assert!(!self.frozen);
        // Can't use with() because object may not exist.
        let ty = Ty::of::<Tracker<E>>();
        let mut objects = self.objects.lock().unwrap();
        let obj = objects
            .entry(ty)
            .or_insert_with(|| Locked::new(
                Box::new(Tracker::<E>::new()),
                type_name::<Tracker<E>>(),
            ));
        obj.acquire(Access::Write);
        unsafe {
            let obj: &mut dyn AnyDebug = &mut *obj.contents();
            let obj: &mut Tracker<E> = obj.downcast_mut().unwrap();
            obj.handlers.push(f);
        }
        obj.release(Access::Write);
    }
}

#[cfg(test)]
mod test_tracking {
    use super::*;

    decl_table! {
        pub struct ships {
            pub name: Name,
            pub weight: u32,
        }
    }

    decl_table! {
        pub struct sailors {
            pub name: Name,
            pub ship: ships::Id,
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
        println!("Starting!");
        let universe = &mut Universe::new();
        ships::Marker::register(universe);
        sailors::Marker::register(universe);
        println!("hello there");
        universe.kmap(|mut ships: ships::Write, mut sailors: sailors::Write| {
            println!("pushing stuff");
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
            println!("stuff pushed");
        });
        println!("first kmap");
        universe.kmap(|ships: ships::Read, sailors: sailors::Read| {
            println!("{:?}", ships.ids());
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
            |ships: &mut ships::Ids, names: ships::read::name, weight: ships::read::weight| {
                let mut sunk = false;
                for f in ships.removing() {
                    if weight[f.id] == 20 {
                        println!("The {} is sinking! Oh, the humanity!", names[f.id]);
                        f.remove();
                        assert!(!sunk);
                        sunk = true;
                    }
                }
                assert!(sunk);
                println!("\nids: {:?}", ships);
            },
        );
        universe.kmap(|ships: ships::Read, sailors: sailors::Read| {
            println!("\nids: {:?}", ships.ids());
            println!("\nAll Ships:");
            let mut count = 0;
            let mut no_boaty = true;
            for id in ships.iter() {
                let ship = ships.ref_row(id);
                println!("{:?} = {:?}", id, ship);
                no_boaty &= !ship.name.contains("Boaty");
                count += 1;
            }
            assert!(no_boaty);
            assert_eq!(count, 3);
            println!("\nSailors:");
            for id in sailors.iter() {
                let sailor = sailors.ref_row(id);
                println!("{:?} = {:?}", id, sailor);
            }
            println!();
            let mut count = 0;
            for id in sailors.iter() {
                let sailor = sailors.ref_row(id);
                println!("{:?} = {:?}", id, sailor);
                assert_ne!(sailor.name, &"Charles");
                assert_ne!(sailor.name, &"Darude");
                count += 1;
            }
            assert_eq!(count, 6 - 2);
        });
    }
}

/// Represents different "lifetimes of data".
// FIXME: Rather game-specific; deserves a feature.
pub mod lifestage {
    /// The values have been added or removed from memory. This is less specific than `Logical`
    /// and `Load`. It is used for things like indices. Has 'RAII' ordering, so it wraps around the
    /// outside of `Logical`and `Load` events.
    #[derive(Debug, Copy, Clone)] pub struct MEMORY;
    /// An object has been created or destroyed.
    #[derive(Debug, Copy, Clone)] pub struct LOGICAL;
    /// This does not represent the actual creation or destruction of an object, but rather simply
    /// that it has been saved to disk.
    #[derive(Debug, Copy, Clone)] pub struct LOAD;
}

/// # Safety
/// Maintaining a sound/coherent/consistent database requires that there be no foreign key pointing
/// at a dead row.
/// A `Push` (or `Delete`) of a `LOGICAL` lifestage must be preceded (or followed) by an
/// otherwise identical `MEMORY` event, so that indices may update (and so that foreign key deletions
/// can cascade down.) However, a `LOAD` must maintain consistency through its own means. A lone
/// `MEMORY` event is of dubious use, and instead must accompany one of the other two.
#[derive(Debug)]
pub struct Unsafe<T>(T);
impl<T> Unsafe<T> {
    pub unsafe fn new(t: T) -> Unsafe<T> { Unsafe(t) }
}

#[derive(Debug)]
pub struct Push<M: TableMarker, Lifestage> {
    pub lifestage: Unsafe<Lifestage>,
    pub ids: RunList<M>,
}
#[derive(Debug)]
pub struct Edit<M: TableMarker, T: AnyDebug> {
    pub(crate) col: *const Column<M, T>,
    pub new: Vec<(Id<M>, T)>,
    // Or this could be split into
    //    new_ids: RunList<M>,
    //    new_values: Vec<T>,
}
unsafe impl<M: TableMarker, T: AnyDebug> Send for Edit<M, T> {}
unsafe impl<M: TableMarker, T: AnyDebug> Sync for Edit<M, T> {}
impl<M: TableMarker, T: AnyDebug> Edit<M, T> {
    pub fn col(&self) -> &Column<M, T> {
        unsafe { &*self.col }
    }
}

#[derive(Debug)]
pub struct Delete<M: TableMarker, Lifestage> {
    pub lifestage: Unsafe<Lifestage>,
    pub ids: RunList<M>,
}

#[cfg(feature = "move_event")]
#[derive(Debug)]
pub struct Move<M: TableMarker> {
    /// (old, new)
    pub ids: Vec<(Id<M>, Id<M>)>,
}
/// Requires `move_event` feature.
#[cfg(not(feature = "move_event"))]
#[derive(Debug)]
pub enum Move {}
