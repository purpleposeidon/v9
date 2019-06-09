use v9::prelude_lib::*;
use v9::column::Column;

#[derive(Default, Debug, Copy, Clone)]
struct M;
impl Register for M {
    fn register(_universe: &mut Universe) { unimplemented!() }
}
impl TableMarker for M {
    const NAME: Name = "TestTable";
    type RawId = u8;
    fn header() -> TableHeader { unimplemented!() }
}


#[test]
fn serialize_it() {
    let col = Column {
        table_marker: M,
        data: vec![true, false, true, true],
    };
    println!("{}", serde_json::to_string_pretty(&col).unwrap());
}
