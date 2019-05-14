v9::table! {
    /// This is my table.
    /// There are many like it.
    #[raw_index(u8)]
    pub struct cheeses {
        /// How much cheese.
        pub quantity: f64,
        /// Stinky: Well, is it?
        pub stinky: bool,
    }
}

#[test]
fn compiles() {}
