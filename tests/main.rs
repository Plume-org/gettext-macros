#![feature(proc_macro_hygiene, decl_macro)]

use gettext_macros::*;

struct Catalog;

impl Catalog {
    pub fn gettext(&self, msg: &'static str) -> &'static str {
        msg
    }

    pub fn ngettext(&self, msg: &'static str, _pl: &'static str, _count: i32) -> &'static str {
        msg
    }
}

#[allow(dead_code)]
fn build() {
    configure_i18n!("test", "po/test", fr, en, de);
}

init_i18n!("test");

#[test]
fn main() {
    let cat = Catalog;
    let x = i18n!(cat, "Hello");
    let b = i18n!(cat, "Singular", "Plural"; 0);
    println!("{} {}", x, b);
}
