use crate::column::*;
use crate::event::*;
use crate::kernel::{Kernel, KernelArg, KernelFn};
use crate::prelude_lib::*;
use std::collections::BTreeMap;

pub struct ColumnIndex<M: TableMarker, T: Ord> {
    pub map: BTreeMap<(T, Id<M>), ()>,
}
impl<M: TableMarker, T: Ord + Clone> ColumnIndex<M, T> {
    pub fn full_range(t: T) -> StdRange<(T, Id<M>)> {
        (t.clone(), Id(M::RawId::ZERO))..(t, Id(M::RawId::LAST))
    }
}
impl<M: TableMarker, T: Ord> Default for ColumnIndex<M, T> {
    fn default() -> Self {
        ColumnIndex {
            map: BTreeMap::new(),
        }
    }
}
impl<M: TableMarker, T: 'static + Send + Sync + Ord> Obj for ColumnIndex<M, T> {}
unsafe impl<'a, M: TableMarker, T: 'static + Send + Sync + Ord> Extract for &'a ColumnIndex<M, T> {
    fn each_resource(f: &mut dyn FnMut(TypeId, Access)) {
        f(TypeId::of::<ColumnIndex<M, T>>(), Access::Read)
    }
    type Owned = Self;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self::Owned {
        rez.take_ref_downcast()
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        *owned
    }
}
unsafe impl<'a, M: TableMarker, T: 'static + Send + Sync + Ord> Extract for &'a mut ColumnIndex<M, T> {
    fn each_resource(f: &mut dyn FnMut(TypeId, Access)) {
        f(TypeId::of::<ColumnIndex<M, T>>(), Access::Write)
    }
    type Owned = Self;
    unsafe fn extract(_universe: &Universe, rez: &mut Rez) -> Self::Owned {
        rez.take_mut_downcast()
    }
    unsafe fn convert(_universe: &Universe, owned: *mut Self::Owned) -> Self {
        *owned
    }
}
impl Universe {
    pub fn add_index<M: TableMarker, T>(&mut self)
    where
        T: 'static + Send + Sync + Ord + Copy,
    {
        // 1. Add the index.
        // Col<M, T>
        // index: Map<(T, Id<M>)>
        self.add_mut(
            TypeId::of::<ColumnIndex<M, T>>(),
            ColumnIndex::<M, T>::default(),
        );
        // Next we add handlers for each event:
        self.tracker_with_arg::<_, _, Pushed<M>>(
            |ev: KernelArg<&Pushed<M>>, index: &mut ColumnIndex<M, T>, local: ReadColumn<M, T>| {
                // 2. Insertion
                // i = col.push(new)
                // new index[(old, i)]
                for id in ev.range {
                    let val = local[id];
                    index.map.insert((val, id), ());
                }
            },
        );
        self.tracker_with_arg::<_, _, Edited<M, T>>(
            |ev: KernelArg<&Edited<M, T>>, index: &mut ColumnIndex<M, T>| {
                // 3. Edit
                // col[i] = new;
                // index[(old, i)] -> index[(new, i)]
                let col = ReadColumn { col: ev.col() };
                for &(id, new) in &ev.new {
                    let old = col[id];
                    // if old == new { continue; }
                    // We could do this check.
                    // But it'd slow down well-written code.
                    index.map.remove(&(old, id));
                    index.map.insert((new, id), ());
                }
            },
        );
        self.tracker_with_arg::<_, _, Deleted<M>>(
            |ev: KernelArg<&Deleted<M>>, index: &mut ColumnIndex<M, T>, col: ReadColumn<M, T>| {
                // 4. Delete
                // del col[i];
                // del index[(old, i)];
                for &id in &ev.ids {
                    let old = col[id];
                    index.map.remove(&(old, id));
                }
            },
        );
        self.tracker_with_arg::<_, _, Moved<M>>(
            |ev: KernelArg<&Moved<M>>, index: &mut ColumnIndex<M, T>, local: ReadColumn<M, T>| {
                // 5. Moved
                // col[i] -> col[j];
                // del index[(val, i)];
                // new index[(val, j)];
                for &(i, j) in &ev.ids {
                    let val = local[j];
                    index.map.remove(&(val, i));
                    index.map.insert((val, j), ());
                }
            },
        );
    }
    fn tracker_with_arg<F, Dump, E>(&mut self, f: F)
    where
        F: KernelFn<Dump>,
        E: Obj,
    {
        let mut kernel = Kernel::new(f);
        self.add_tracker(move |universe: &Universe, ev: &E| {
            kernel.push_arg(ev);
            universe.run(&mut kernel);
        });
    }
}

/// This is a ducktyping-style hack used in lieu of specialization
/// (which is still unstable). If your type is a foreign key, you should
/// implement a function with the same name as the one in this trait.
pub trait ForeignKey {
    fn __v9_link_foreign_key<LM: TableMarker>(_universe: &mut Universe) {}
}
impl<X> ForeignKey for X {}
impl<FM: TableMarker> Id<FM> {
    pub fn __v9_link_foreign_key<LM: TableMarker>(universe: &mut Universe) {
        universe.add_index::<LM, Self>();
        universe.tracker_with_arg::<_, _, Deleted<FM>>(
            |ev: KernelArg<&Deleted<FM>>, list: &mut IdList<LM>, index: &ColumnIndex<LM, Self>| {
                // 6. Use the index to decide which IDs get the axe.
                let deleting = list.deleting.get_mut();
                deleting.reserve(ev.ids.len());
                // We won't reserve enough space if the local table has multiple references to a
                // single foreign row.
                for &fid in &ev.ids {
                    let range = ColumnIndex::full_range(fid);
                    let locals = index.map.range(range);
                    deleting.extend(locals.map(|((_fid, lid), ())| *lid));
                }
            },
        );
        universe.tracker_with_arg::<_, _, Moved<FM>>(
            |ev: KernelArg<&Moved<FM>>, index: &ColumnIndex<LM, Self>, mut col: EditColumn<LM, Self>| {
                // 7. Use the index to update everyone point at moved things.
                // The index also needs to be updated.
                // It'll take care of itself after the kernel finishes.
                for &(ofid, nfid) in &ev.ids {
                    for (&(_, id), ()) in index.map.range(ColumnIndex::full_range(ofid)) {
                        col[id] = nfid;
                    }
                }
            },
        );
    }
}
