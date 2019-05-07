v9::table! {
    pub struct boats {
        pub name: &'static str,
    }
}

#[test]
fn compiles_without_orphan_trouble() {}
