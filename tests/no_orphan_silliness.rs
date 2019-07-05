v9::decl_table! {
    pub struct boats {
        pub name: &'static str,
    }
}

#[test]
fn compiles_without_orphan_trouble() {}
