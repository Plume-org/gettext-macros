# Gettext macros

A few proc-macros to help you internationalize your Rust apps.

## How does it works?

There are two main macros and a function:

- `compile_i18n`, that initializes and updates the translation files.
  It must be called from a [build script][bscript].
- `i18n!`, that translates a given message.
- `include_i18n!`, that will embed translations in your binary, making it easier to distribute.

The advantage of these macros is that they allow you to work with multiple translation
domains (for instance, one for each of your workspace's crate), and that they automatically
generate a .pot file for these domains.

[bscript]: https://doc.rust-lang.org/cargo/reference/build-scripts.html

## Example

*Cargo.toml*

```toml
[dependencies]
gettext = "*"
gettext-macros = "*"
gettext-utils = "*"

[build-dependencies]
gettext-utils = "*"
```

*build.rs*

```rust
extern crate gettext_utils;

use gettext_utils::compile_i18n;

fn main() {
    // The translations for this crate are stored in the "my_app" domain.
    // Translations for all the listed langages will be available.
    compile_i18n("my_app", &["ar", "de", "en", "fr", "it", "ja", "ru"]);
}
```

*src/main.rs*

```rust
extern crate gettext;
extern crate gettext_macros;
extern crate gettext_utils;

use gettext_macros::{include_i18n, i18n};

fn main() {
    // include_i18n! embeds translations in your binary.
    // It gives a Vec<(&'static str, Catalog)> (list of catalogs with their associated language).
    let (language, catalog) = include_i18n!()[0];

    println!("{}", i18n!(catalog, "Hello, world!"));
    let name = "Jane";
    println!("{}", i18n!(catalog, "Hello, {}!"; name));
    let message_count = 42;
    println!("{}", i18n!(catalog, "You have one new message", "You have {0} new messages"; message_count));
}
```

## TODO

- Format args checking