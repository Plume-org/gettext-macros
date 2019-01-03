# Gettext macros

A few proc-macros to help you internationalize your Rust apps.

## How does it works?

There are three macros:

- `configure_i18n`, that should be in your `build.rs` file. It
allows you to configure multiple translation domains, where to store
their .pot file and which langage they support.
- `init_i18n`, that should be called in your crate root. It tells which
domain to use for this crate.
- `i18n`, that translates a given message

The advantage of these macros is that they allow you to work with multiple translation
domains (for instance, one for each of your workspace's crate), and that they automatically
generate a .pot file for these domains.

## Example

*build.rs*

```rust
use gettext_macros::configure_i18n;

fn main() {
    // Configure two different translation domains, with different locales
    configure_i18n!("my_app", ar, en, de, fr, it, ja); // This one will have its translations stored in ./po
    configure_i18n!("another_domain", "po/other_domain", en, de, ja); // This one in ./po/other_domain
}
```

*main.rs*

```rust
// The translations for this module and its submodules will be
// loaded from the "my_app" domain.
init_i18n!("my_app");

fn init_catalog() -> gettext::Catalog {
    // return the correct catalog for the user's language,
    // usually with another crate.
}

fn main() {
    let catalog = init_catalog();
    println!("{}", i18n!(catalog, "Hello, world!"));
    let name = "Jane";
    println!("{}", i18n!(catalog, "Hello, {}!"; name));
    let message_count = 42;
    println!("{}", i18n!(catalog, "You have one new message", "You have {0} new messages"; message_count));
}
```

## TODO

- Code cleanup
- Format args checking
- Use package name as default domain
- Add build functions to gettext-utils
