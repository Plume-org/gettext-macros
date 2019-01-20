# Gettext macros

A few proc-macros to help you internationalize your Rust apps.

## How does it works?

There are four main macros:

- `init_i18n`, that should be called first. It tells the domain to use for the current
crate, and the supported locales.
- `include_i18n`, that will embed translations in your binary, making it easier to distribute.
- `compile_i18n`, that should be called at the end of your `main.rs`. It updates translation files and compile them.
- `i18n`, that translates a given message.

The advantage of these macros is that they allow you to work with multiple translation
domains (for instance, one for each of your workspace's crate), and that they automatically
generate a .pot file for these domains.

## Example

*main.rs*

```rust
// The translations for this crate are stored in the "my_app" domain.
// Translations for all the listed langages will be available.
init_i18n!("my_app", ar, de, en, fr, it, ja, ru);

fn main() {
    // include_i18n! embeds translations in your binary.
    // It gives a Vec<(&'static str, Catalog)> (list of catalogs with their associated language).
    let catalog = include_i18n!()[0];

    println!("{}", i18n!(catalog, "Hello, world!"));
    let name = "Jane";
    println!("{}", i18n!(catalog, "Hello, {}!"; name));
    let message_count = 42;
    println!("{}", i18n!(catalog, "You have one new message", "You have {0} new messages"; message_count));
}

// Generate or update .po from .pot, and compile them to .mo
compile_i18n!();
```

## TODO

- Format args checking