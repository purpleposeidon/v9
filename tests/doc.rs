v9::table! {
    pub struct cheeses {
        /// How much cheese.
        pub quantity: f64,
        /// Stinky: Well, is it?
        pub stinky: bool,
    }
}

#[test]
fn compiles() {}
