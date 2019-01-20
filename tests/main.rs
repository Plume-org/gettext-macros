#![feature(proc_macro_hygiene, decl_macro, uniform_paths)]

use gettext_macros::*;

init_i18n!("test", fr, en, de, ja);

#[test]
fn main() {
    let catalogs = include_i18n!();
    let cat = &catalogs[0];
    let x = i18n!(cat, "Hello");
    let b = i18n!(cat, "Singular", "Plural"; 0);
    println!("{} {}", x, b);
    println!("{}", i18n!(cat, "Woohoo, it {}"; "works"));
    println!(i18n_domain!());

}

compile_i18n!();
