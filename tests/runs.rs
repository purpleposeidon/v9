extern crate rand;
extern crate v9;
use rand::Rng;

use v9::prelude_lib::*;

#[derive(Debug, Copy, Clone, Default)]
struct M;
impl TableMarker for M {
    const NAME: Name = "";
    type RawId = u32;
    fn header() -> TableHeader { unimplemented!() }
}
impl Register for M {
    fn register(_universe: &mut Universe) { unimplemented!() }
}


#[allow(unused_macros)]
#[test]
fn log1() {
    let mut ids = IdList::<M>::default();
    let u = Universe::new();
    macro_rules! recycle_ids_contiguous {
        ($n:expr, $expect:expr) => {
            let n = $n;
            let _r = unsafe { ids.recycle_ids_contiguous(n, true) };
            //let expect = $expect;
            //assert_eq!(r, expect);
        };
    }
    macro_rules! recycle_ids {
        ($n:expr, $expect:expr) => {
            let n = $n;
            let _r = unsafe { ids.recycle_ids(n, true) };
            //let expect = $expect;
            //assert_eq!(r, expect);
        };
    }
    macro_rules! remove {
        ($run:expr) => {
            let run = $run;
            let run = (Id::<M>::new(run.start()[0]))..=(Id::<M>::new(run.end()[0]));
            ids.delete_extend_ranges(Some(run).into_iter());
        };
    }
    {
        recycle_ids_contiguous!(6, Recycle { replace: [](len=0), extend: 6, extension: [0..6] });
        // IdList { free: [](len=0), pushing: [](len=0), deleting: SyncRef { val: RefCell { value: [](len=0) } }, outer_capacity: 6 }
        ids.flush(&u);
        // IdList { free: [](len=0), pushing: [](len=0), deleting: SyncRef { val: RefCell { value: [](len=0) } }, outer_capacity: 6 }

        remove!([0]..=[5]);
        // IdList { free: [](len=0), pushing: [](len=0), deleting: SyncRef { val: RefCell { value: [[0]..=[5]](len=6) } }, outer_capacity: 6 }
        ids.flush(&u);
        // IdList { free: [[0]..=[5]](len=6), pushing: [](len=0), deleting: SyncRef { val: RefCell { value: [](len=0) } }, outer_capacity: 6 }

        recycle_ids!           (3, Recycle { replace: [[0]..=[2]](len=3), extend: 0, extension: [0..0] });
    }
    ids.flush(&u);
}

#[test]
fn fuzz() {
    let mut ids = IdList::<M>::default();
    let mut rng = rand::thread_rng();
    let u = Universe::new();

    let mut runs = vec![];
    for _ in 0..100 {
        let n = rng.gen_range(1..5usize);
        if rng.gen() {
            for _ in 0..n {
                let n = rng.gen_range(1..10usize);
                let recycle = if rng.gen() {
                    let r = unsafe { ids.recycle_ids_contiguous(n, true) };
                    println!("recycle_ids_contiguous!({}, {:?});", n, r);
                    r
                } else {
                    let r = unsafe { ids.recycle_ids(n, true) };
                    println!("recycle_ids!           ({}, {:?});", n, r);
                    r
                };
                for run in recycle.replace.iter_runs() {
                    runs.push(run);
                }
                if !recycle.extension.is_empty() {
                    let run = recycle.extension.start ..= recycle.extension.end.step(-1);
                    runs.push(run.into());
                }
            }
        } else {
            for _ in 0..n {
                if runs.is_empty() { break; }
                let run = rng.gen_range(0..=runs.len()-1);
                let run: IdRange::<Id<M>> = runs.remove(run);
                println!("remove!({:?});", run);
                let run: std::ops::RangeInclusive<Id<M>> = run.into();
                ids.delete_extend_ranges(Some(run).into_iter());
            }
        }
        println!("// {:?}", ids);
        println!("ids.flush(&u, false, false);");
        ids.flush(&u);
        println!("// {:?}\n", ids);
    }
}
