#![feature(proc_macro_hygiene, decl_macro, uniform_paths)]

use gettext_macros::*;

init_i18n!("test", fr, en, de, ja);

#[test]
fn main() {
    let cat = get_i18n();
    let x = i18n!(cat, "Hello");
    let b = i18n!(cat, "Singular", "Plural"; 0);
    i18n!(cat, context = "Test context", "Hello");
	i18n!(cat, context = "Test context (plural)", "Hello", "Plural"; 2);
	i18n!(cat, context = "Test context (format)", "Hello {}"; "world");
    println!("{} {}", x, b);
    println!("{}", i18n!(cat, "Woohoo, it {}"; "works"));
    println!(i18n_domain!());
}

compile_i18n!();

fn get_i18n() -> gettext::Catalog {
    include_i18n!()[0].1.clone()
}
