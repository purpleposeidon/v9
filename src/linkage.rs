//! Connecting tables.
use crate::column::*;
use crate::event::*;
use crate::kernel::{Kernel, KernelArg, KernelFn};
use crate::prelude_lib::*;
use crate::id::IdRange;
use std::collections::{BTreeMap, HashMap};
use std::any::{Any, TypeId};
use std::mem;

pub type IndexOf<C> = ColumnIndex<
    <C as LiftColumn>::M,
    <C as LiftColumn>::T,
>;
#[doc(hidden)]
pub trait LiftColumn {
    type M;
    type T;
}
impl<M: TableMarker, T> LiftColumn for Column<M, T> {
    type M = M;
    type T = T;
}


pub struct ColumnIndex<M: TableMarker, T: Ord> {
    pub map: BTreeMap<(T, Id<M>), ()>,
}
impl<M: TableMarker, T: Ord + Clone> ColumnIndex<M, T> {
    pub fn full_range(t: T) -> StdRange<(T, Id<M>)> {
        (t.clone(), Id(M::RawId::ZERO))..(t, Id(M::RawId::LAST))
    }
    pub fn find<'a>(&'a self, t: T) -> impl DoubleEndedIterator<Item=Id<M>> + 'a {
        self.map
            .range(Self::full_range(t))
            .map(|((_, i), _)| *i)

    }
}
impl<M: TableMarker, T: Ord> Default for ColumnIndex<M, T> {
    fn default() -> Self {
        ColumnIndex {
            map: BTreeMap::new(),
        }
    }
}
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
    type Cleanup = ();
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
    type Cleanup = ();
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
        self.add_tracker_with_ref_arg::<_, _, Pushed<M>>(
            |ev: KernelArg<&Pushed<M>>, index: &mut ColumnIndex<M, T>, local: ReadColumn<M, T>| {
                // 2. Insertion
                // i = col.push(new)
                // new index[(old, i)]
                for id in &ev.ids {
                    let val = local[id];
                    index.map.insert((val, id), ());
                }
            },
        );
        self.add_tracker_with_ref_arg::<_, _, Edited<M, T>>(
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
        self.add_tracker_with_ref_arg::<_, _, Deleted<M>>(
            |ev: KernelArg<&Deleted<M>>, index: &mut ColumnIndex<M, T>, col: ReadColumn<M, T>| {
                // 4. Delete
                // del col[i];
                // del index[(old, i)];
                for id in &ev.ids {
                    let old = col[id];
                    index.map.remove(&(old, id));
                }
            },
        );
        self.add_tracker_with_ref_arg::<_, _, Moved<M>>(
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
    pub fn add_tracker_with_ref_arg<F, Dump, E>(&mut self, f: F)
    where
        F: KernelFn<Dump, ()>,
        F: 'static + Send + Sync,
        E: Any + Send + Sync,
        Dump: Send + Sync,
    {
        let mut kernel = Kernel::new(f);
        self.add_tracker(move |universe: &Universe, ev: &mut E| {
            kernel.push_arg(ev);
            universe.run(&mut kernel);
        });
    }
    pub fn add_tracker_with_mut_arg<F, Dump, E>(&mut self, f: F)
    where
        F: KernelFn<Dump, ()>,
        F: 'static + Send + Sync,
        E: Any + Send + Sync,
        Dump: Send + Sync,
    {
        let mut kernel = Kernel::new(f);
        self.add_tracker(move |universe: &Universe, ev: &mut E| {
            kernel.push_arg_mut(ev);
            universe.run(&mut kernel);
        });
    }
}

/// This is a ducktyping-style hack used in lieu of specialization
/// (which is still unstable). If your type is a foreign key, you should
/// implement a function with the same name as the one in this trait.
pub trait ForeignKey {
    fn __v9_link_foreign_table_name() -> Option<Name> { None }
    fn __v9_link_foreign_key<LM: TableMarker>(_universe: &mut Universe) {}
}
impl<X> ForeignKey for X {}
impl<FM: TableMarker> Id<FM> {
    pub fn __v9_link_foreign_table_name() -> Option<Name> {
        Some(FM::NAME)
    }
    pub fn __v9_link_foreign_key<LM: TableMarker>(universe: &mut Universe) {
        if TypeId::of::<LM>() == TypeId::of::<FM>() {
            // You're on your own.
            return;
        }
        universe.add_index::<LM, Self>();
        universe.add_tracker_with_ref_arg::<_, _, Deleted<FM>>(
            |ev: KernelArg<&Deleted<FM>>, list: &mut IdList<LM>, index: &ColumnIndex<LM, Self>| {
                // 6. Use the index to decide which IDs get the axe.
                // We won't reserve enough space if the local table has multiple references to a
                // single foreign row.
                list.delete_extend(
                    ev.ids
                        .iter()
                        .flat_map(|fid| {
                            let range = ColumnIndex::full_range(fid);
                            let locals = index.map.range(range);
                            locals.into_iter().map(|((_fid, lid), ())| *lid)
                        })
                );
            },
        );
        universe.add_tracker_with_ref_arg::<_, _, Moved<FM>>(
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
        let mut is_tracked: Option<bool> = None;
        universe.add_tracker_with_mut_arg::<_, _, Select<FM>>(
            move |mut ev: KernelArg<&mut Select<FM>>, index: &ColumnIndex<LM, Self>, universe: UniverseRef| {
                // 8. Push the local ids of the foreign ids; we have them indexed.
                let foreign: &RunList<FM> = if let Some(f) = ev.selection.get() { f } else { return; };
                let mut got = vec![];
                for fid in foreign.iter() {
                    for lid in index.find(fid) {
                        got.push(lid);
                    }
                }
                if got.is_empty() { return; }
                got.sort();
                // FIXME: See id.rs/timsort. 1) Are these runs? 2) Is timsort faster than unstable?
                got.dedup();
                let mut out: Box<RunList<LM>> = ev.selection.ordered();
                for i in got.into_iter() {
                    out.push(i);
                }
                ev.selection.deliver(out);
                {
                    if is_tracked.is_none() {
                        is_tracked = Some(universe.is_tracked::<Select<LM>>());
                    }
                    if let Some(false) = is_tracked { return; }
                    let mut sub: Select<LM> = Default::default();
                    mem::swap(&mut sub.selection, &mut ev.selection);
                    universe.submit_event(&mut sub);
                    mem::swap(&mut sub.selection, &mut ev.selection);
                }
            },
        );
    }
}
impl<FM: TableMarker> IdRange<'static, Id<FM>> {
    pub fn __v9_link_foreign_table_name() -> Option<Name> {
        Some(FM::NAME)
    }
    pub fn __v9_link_foreign_key<LM: TableMarker>(universe: &mut Universe) {
        if TypeId::of::<LM>() == TypeId::of::<FM>() {
            // You're on your own.
            return;
        }
        universe.add_index::<LM, Self>();
        universe.add_tracker_with_ref_arg::<_, _, Deleted<FM>>(
            |ev: KernelArg<&Deleted<FM>>, list: &mut IdList<LM>, index: &ColumnIndex<LM, Self>| {
                let mut prev = IdRange::empty();
                for fid in &ev.ids {
                    if prev.contains(fid) {
                        // We've already removed this ID.
                        continue;
                    }
                    let range = {
                        let ll = Id(LM::RawId::LAST);
                        let fl = Id(FM::RawId::LAST);
                        let back = (IdRange::new(fid, fl), ll);
                        ..back
                    };
                    let mut iter = index.map.range(range);
                    // Option<(&(IdRange<Id<FM>>, Id<LM>), &())>
                    while let Some(((frange, lid), ())) = iter.next_back() {
                        if frange.contains(fid) {
                            prev = *frange;
                            list.delete(*lid);
                        } else {
                            break;
                        }
                    }
                }
            },
        );
        // FIXME: 'Moved' is kinda hard. :/
        let mut is_tracked: Option<bool> = None;
        universe.add_tracker_with_mut_arg::<_, _, Select<FM>>(
            move |mut ev: KernelArg<&mut Select<FM>>, index: &ColumnIndex<LM, Self>, universe: UniverseRef| {
                // 8. Push the local ids of the foreign ids; we have them indexed.
                let foreign: &RunList<FM> = if let Some(f) = ev.selection.get() { f } else { return; };
                let mut got = vec![];
                let mut prev = IdRange::empty();
                for fid in foreign.iter() {
                    if prev.contains(fid) {
                        // We've already removed this ID.
                        continue;
                    }
                    let range = {
                        let ll = Id(LM::RawId::LAST);
                        let fl = Id(FM::RawId::LAST);
                        let back = (IdRange::new(fid, fl), ll);
                        ..back
                    };
                    let mut iter = index.map.range(range);
                    while let Some(((frange, lid), ())) = iter.next_back() {
                        if frange.contains(fid) {
                            prev = *frange;
                            got.push(*lid);
                        } else {
                            break;
                        }
                    }
                }
                if got.is_empty() { return; }
                got.sort();
                // FIXME: See id.rs/timsort. 1) Are these runs? 2) Is timsort faster than unstable?
                got.dedup();
                let mut out: Box<RunList<LM>> = ev.selection.ordered();
                for i in got.into_iter() {
                    out.push(i);
                }
                ev.selection.deliver(out);
                {
                    if is_tracked.is_none() {
                        is_tracked = Some(universe.is_tracked::<Select<LM>>());
                    }
                    if let Some(false) = is_tracked { return; }
                    let mut sub: Select<LM> = Default::default();
                    mem::swap(&mut sub.selection, &mut ev.selection);
                    universe.submit_event(&mut sub);
                    mem::swap(&mut sub.selection, &mut ev.selection);
                }
            },
        );
    }
}
// FIXME: We could do RunList as well

/// Holds a bunch of `RunList`s.
#[derive(Debug, Default)]
pub struct Selection {
    pub seen: HashMap<TypeId, Box<Any + Send + Sync>>,
    pub selection_order: Vec<TypeId>,
}
impl Selection {
    pub fn get<M: TableMarker>(&self) -> Option<&RunList<M>> {
        let ty = TypeId::of::<M>();
        self.seen.get(&ty)
            .and_then(|a| a.downcast_ref())
    }
    pub fn ordered<M: TableMarker>(&mut self) -> Box<RunList<M>> {
        let ty = TypeId::of::<M>();
        self.seen.remove(&ty)
            .and_then(|a| {
                (a as Box<Any>).downcast().ok()
            })
            .unwrap_or_else(Default::default)
    }
    pub fn deliver<M: TableMarker>(&mut self, ids: Box<RunList<M>>) {
        let ty = TypeId::of::<M>();
        self.seen.insert(ty, ids);
        self.selection_order.push(ty);
    }
    pub fn from<FM: TableMarker>(sel: RunList<FM>) -> Self {
        let mut seen = HashMap::new();
        seen.insert(TypeId::of::<FM>(), Box::new(sel) as Box<Any + Send + Sync>);
        Selection { seen, selection_order: vec![] }
    }
    pub fn add_stub<T: Any>(&mut self) {
        let ty = TypeId::of::<T>();
        self.seen.insert(ty, Box::new(()));
        self.selection_order.push(ty);
    }
    pub fn deselect(&mut self, ty: TypeId) {
        self.seen.remove(&ty);
        self.selection_order.retain(|&t| t != ty);
    }
}
#[derive(Default)]
pub struct Select<FM> {
    pub foreign_marker: FM,
    pub selection: Selection,
}
impl<FM: TableMarker> Select<FM> {
    pub fn from(sel: RunList<FM>) -> Self {
        Select {
            foreign_marker: FM::default(),
            selection: Selection::from(sel)
        }
    }
}
