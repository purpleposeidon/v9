
#[v9::table]
pub struct root_table {
    pub integer: i32,
    pub bar: crate::bar::bar_table::Id,
}

pub mod foo {
    #[v9::table]
    pub struct foo_table {
        pub root1: crate::root_table::Id,
        //pub root2: super::root_table::Id,
    }
}

pub mod bar {
    #[v9::table]
    pub struct bar_table {
        pub foo: i32,
    }
}
