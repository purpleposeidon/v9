//! Attribute macro wrappers for `v9`'s `decl_` series of macros.

extern crate proc_macro;
use crate::proc_macro::*;
use std::str::FromStr;

fn make(name: &str, input: TokenStream) -> TokenStream {
    // Not sure why this doesn't work.
    /*
    let mut out = TokenStream::new();
    out.extend(vec![
        TokenTree::Ident(Ident::new("v9", span)),
        TokenTree::Punct(Punct::new(':', Spacing::Joint)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new(name, span)),
        TokenTree::Punct(Punct::new('!', Spacing::Alone)),
        TokenTree::Group(Group::new(Delimiter::Brace, input.clone())),
    ]);
    */
    let ret = FromStr::from_str(&format!("v9::{}! {{ {} }}", name, input)).unwrap();
    //println!("{:#?}", ret);
    ret
}

// FIXME: Use Span::def_site().

/// Wrapper around [`v9::decl_table!`](../v9/macro.decl_table.html).
#[proc_macro_attribute]
pub fn table(_attr: TokenStream, input: TokenStream) -> TokenStream {
    make("decl_table", input)
}

/// Wrapper around [`v9::decl_context!`](../v9/macro.decl_context.html).
#[proc_macro_attribute]
pub fn context(_attr: TokenStream, input: TokenStream) -> TokenStream {
    make("decl_context", input)
}

/// A *sorta* wrapper around [`v9::decl_property!`](../v9/macro.decl_property.html).
/// There are two complications:
/// 1. This is pretty much inherently only going to work on local types, so the `~i32` thing doesn't work.
///
/// 2. The struct must `impl Default`. (Well, I guess there could be a `struct Foo {} = init;` thing,
/// but that'd look weird!)
#[proc_macro_attribute]
pub fn property(_attr: TokenStream, input: TokenStream) -> TokenStream {
    // #[property(cheese_db)]
    // pub struct Cheeses;
    let mut vis = TokenStream::new();
    let mut hit_struct = false;
    let mut struct_name = None;
    for t in input.clone().into_iter() {
        // '#'? Skip. '[]'? Skip.
        // 'struct'? The name follows.
        // Anything else? Prolly the visibility
        match t {
            TokenTree::Punct(ref p) if p.as_char() == '#' => (),
            TokenTree::Group(ref g) if g.delimiter() == Delimiter::Bracket => (),
            TokenTree::Ident(ref i) if &i.to_string() == "struct" => hit_struct = true,
            TokenTree::Ident(ref i) if hit_struct => {
                struct_name = Some(i.clone());
                break;
            },
            t => vis.extend(Some(t)),
        }
    }
    let struct_name = struct_name.expect("expected 'struct name' or something");
    let out = format!(r#"
{input}
mod _v9_property_call_{name} {{
    type TheType = super::{name};
    v9::decl_property! {{ {vis} {name}: TheType }}
}}
"#, input=input, vis=vis, name=struct_name);
    let ret = FromStr::from_str(&out).unwrap();
    ret
}
