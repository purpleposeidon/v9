use v9::prelude::*;
use v9::kernel::*;
use v9::prelude_lib::*;

#[test]
fn compiles() {
    let mut val = 0;
    let mut k = Kernel::new(move |_u: UniverseRef| {
        val += 1;
        println!("{}", val);
    });
    let u = Universe::new();
    for _ in 0..10 {
        u.run(&mut k);
    }
}

#[test]
fn eval() {
    let u = Universe::new();
    let mut buffer = format!("");
    u.eval(|_u: UniverseRef| {
        buffer += "test!";
    });
    assert_eq!(buffer, "test!");
}

/*
#[test]
fn static_stuff_shouldnt_compile() {
    let mut dude = Option::<&Universe>::None;
    let u = Universe::new();
    u.eval(|verse: UniverseRef<'static>| {
        dude = Some(&verse);
    });
    std::mem::drop(u);
    dude.unwrap().eval(|_verse: UniverseRef| {
        panic!();
    });
}
*/

#[test]
fn borrowing_universe() {
    let owo = Universe::new();
    owo.eval(|_this: UniverseRef| {
    });
}
