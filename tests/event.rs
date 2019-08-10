use v9::prelude_lib::*;
use v9::kernel::*;
use v9::event::*;

v9::decl_table! {
    struct dudes {
        pub dudeitude: u64,
    }
}

v9::decl_property! {
    pub BOMB_PRIMED: ~bool = true;
}

#[test]
fn track_edit() {
    let mut universe = Universe::new();
    self::dudes::Marker::register(&mut universe);
    self::BOMB_PRIMED::register(&mut universe);
    universe.add_tracker_with_ref_arg::<_, _, Edited<self::dudes::Marker, u64>>(|ev: KernelArg<&Edited<self::dudes::Marker, u64>>, bomb: &mut BOMB_PRIMED| {
        println!("Tracking our dudes");
        for (_id, new) in &ev.new {
            assert_eq!(*new, 100);
        }
        **bomb = false;
    });
    universe.eval(|mut dudes: self::dudes::Write| {
        println!("Pushing some dudes");
        dudes.push(self::dudes::Row {
            dudeitude: 10,
        });
        dudes.push(self::dudes::Row {
            dudeitude: 10,
        });
    });
    universe.eval(|mut dudes: self::dudes::Edit, iter: &self::dudes::Ids| {
        println!("Editing our dudes");
        for dude in iter {
            dudes.dudeitude[dude] = 100;
        }
    });
    universe.with(|bomb: &BOMB_PRIMED| {
        assert!(!**bomb);
    });
}


#[test]
fn track_removal() {
    let mut universe = Universe::new();
    self::dudes::Marker::register(&mut universe);
    self::BOMB_PRIMED::register(&mut universe);
    universe.add_tracker_with_ref_arg::<_, _, Deleted<self::dudes::Marker>>(|_ev: KernelArg<&Deleted<self::dudes::Marker>>, bomb: &mut BOMB_PRIMED| {
        assert!(**bomb, "whack.");
        println!("dude. he died. defusing the bomb. for us. dude had a lot of dudeitude, dude.");
        **bomb = false;
    });
    let check_defused = |rearm| {
        universe.eval(|bomb: &mut BOMB_PRIMED| {
            if **bomb {
                panic!("Oh no! Duuuude!");
            }
            if rearm {
                **bomb = true;
                println!("Whoa, dude! That thing's armed!");
            }
        });
    };
    let check_armed = || {
        universe.eval(|bomb: &BOMB_PRIMED| {
            assert!(**bomb, "gnarly");
        });
    };
    println!("first there was just a whole lotta nothin man");
    check_armed();
    universe.eval(|mut dudes: self::dudes::Write| {
        println!("suddenly two dudes");
        dudes.push(self::dudes::Row {
            dudeitude: 900000000000000,
        });
        dudes.push(self::dudes::Row {
            dudeitude: 900000000000000,
        });
    });
    println!("and they were chill");
    check_armed();
    println!("but then oh no suddenly some serious drama goes down. big bomb.");
    universe.eval(|dude_ids: &mut self::dudes::Ids| {
        for dude in dude_ids.removing() {
            dude.remove();
            break;
        }
    });
    check_defused(true);
    println!("but then he COME SBACK FROM THE DEAD WOAAAA DUUUDE!!!!");
    universe.eval(|mut dudes: self::dudes::Write| {
        let dude_id = dudes.push(self::dudes::Row {
            dudeitude: 900000000000000_0000,
        });
        assert_eq!(dude_id, dudes::FIRST);
    });
    check_armed();
    println!("OH NO THE BOMB AGAIN");
    universe.eval(|dude_ids: &mut self::dudes::Ids| {
        for dude in dude_ids.removing() {
            dude.remove();
            break;
        }
    });
    check_defused(false);
    println!("Better than Shakespeare. Fight me.");
}
