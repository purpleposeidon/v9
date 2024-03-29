use v9::prelude_lib::*;
use v9::kernel::Kernel;

#[v9::table]
pub struct char_list {
    pub c: char,
}

#[v9::table]
pub struct names {
    pub slice: char_list::Range,
}

#[test]
fn test() {
    let universe = &mut Universe::new();
    char_list::Marker::register(universe);
    names::Marker::register(universe);
    let dump = &mut Kernel::new(|chars: char_list::Read, names: names::Read| {
        println!("chars");
        for i in chars.iter() {
            println!("{:?}", chars.c[i]);
        }
        println!("names");
        for i in names.iter() {
            let mut out = String::new();
            for i in names.slice[i] {
                out.push(chars.c[i]);
            }
            println!("{:?} = {:?}", names.slice[i], out);
        }
        println!();
    });
    universe.run(dump);
    universe.kmap(|mut chars: char_list::Write, mut names: names::Write| {
        let data = &["bob", "fred", "steve"];
        for d in data {
            let mut start = None;
            let mut end = char_list::FIRST;
            for c in d.chars() {
                end = chars.push(char_list::Row { c });
                if start.is_none() {
                    start = Some(end);
                }
            }
            let start = start.unwrap();
            names.push(names::Row { slice: (start..end.next()).into() });
        }
    });
    universe.run(dump);
    {
        println!("Delete the first character, 'b'");
        universe.kmap(|chars: &mut char_list::Ids| {
            chars.removing().next().unwrap().remove();
        });
        println!("And now we have...");
        universe.run(dump);
        universe.kmap(|chars: char_list::Read, names: names::Read| {
            let mut out = String::new();
            for i in names.slice[names.iter().next().unwrap()] {
                out.push(chars.c[i]);
            }
            assert_eq!(out.as_str(), "fred");
        });
    }
    {
        println!("Deleting the last name.");
        universe.kmap(|names: &mut names::Ids| {
            for i in names.removing().skip(1) {
                i.remove();
                break;
            }
        });
        universe.run(dump);
        universe.kmap(|names: names::Read| {
            assert_eq!(names.iter().count(), 1);
        });
    }
}
