#![feature(proc_macro_hygiene, proc_macro_quote, proc_macro_span)]

extern crate proc_macro;

use proc_macro::{Delimiter, Literal, Punct, quote, Spacing, TokenStream, TokenTree};
use std::{
    env,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    iter::FromIterator,
};
use gettext_utils as utils;

fn is(t: &TokenTree, ch: char) -> bool {
    match t {
        TokenTree::Punct(p) => p.as_char() == ch,
        _ => false,
    }
}

fn is_empty(t: &TokenTree) -> bool {
	match t {
		TokenTree::Literal(lit) => format!("{}", lit).len() == 2,
		TokenTree::Group(grp) => if grp.delimiter() == Delimiter::None {
			grp.stream().into_iter().next().map(|t| is_empty(&t)).unwrap_or(false)
		} else {
			false
		},
		_ => false,
	}
}

fn trim(t: TokenTree) -> TokenTree {
	match t {
		TokenTree::Group(grp) => if grp.delimiter() == Delimiter::None {
			grp.stream().into_iter().next().expect("Unexpected empty expression")
		} else {
			TokenTree::Group(grp)
		},
		x => x
	}
}

/// Translates the given string with the given catalog.
///
/// This macro will detect when new strings are added in the translations, and
/// produce a warning. To include your new strings in the binary, you will need
/// to build your crate a second time.
///
/// ## Example
///
/// Given you have the correct translations in your `.po` files:
///
/// ```rust,ignore
/// let (_language, ref french_catalog) = &include_i18n!()[0];
/// i18n!(french_catalog, "Hello!");  // "Salut !"
/// ```
///
/// The `0` index in the first line means it is the language listed first in the
/// call of `gettext_utils::compile_i18n`.
///
/// ## Note
///
/// This macro will panic if `gettext_utils::compile_i18n` hasn't been called
/// in the build script.
#[proc_macro]
pub fn i18n(input: TokenStream) -> TokenStream {
    let span = input
        .clone()
        .into_iter()
        .next()
        .expect("Expected catalog")
        .span();
    let mut input = input.into_iter();
    let catalog = input
        .clone()
        .take_while(|t| !is(t, ','))
        .collect::<Vec<_>>();

    let file = span.source_file().path();
    let line = span.start().line;
    let domain = get_current_domain();
    let pot = utils::pot_path(&domain);

    if env::var("GETTEXT_POT_INIT").is_err() {
        utils::init_pot(&pot, &domain).expect("Failed to create the .pot file");
        env::set_var("GETTEXT_POT_INIT", "1");
    }

    let mut pot = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(pot)
        .expect("Couldn't open .pot file");

    for _ in 0..(catalog.len() + 1) {
        input.next();
    }
    let message = trim(input.next().unwrap());

    let mut contents = String::new();
    pot.read_to_string(&mut contents).unwrap();
    pot.seek(SeekFrom::End(0)).unwrap();

    let already_exists = is_empty(&message) || contents.contains(&format!("msgid {}", message));

    let plural = match input.clone().next() {
        Some(t) => {
            if is(&t, ',') {
                input.next();
                input.next()
            } else {
                None
            }
        }
        _ => None,
    };

    let mut format_args = vec![];
    if let Some(TokenTree::Punct(p)) = input.next().clone() {
        if p.as_char() == ';' {
            loop {
                let mut tokens = vec![];
                loop {
                    if let Some(t) = input.next().clone() {
                        if !is(&t, ',') {
                            tokens.push(t);
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                if tokens.is_empty() {
                    break;
                }
                format_args.push(TokenStream::from_iter(tokens.into_iter()));
            }
        }
    }

    let mut res = TokenStream::from_iter(catalog);
    let code_path = if !file.is_absolute() {
    	format!("# {}:{}\n", file.to_str().unwrap(), line)
    } else {
    	String::new()
    };
    if let Some(pl) = plural {
        if !already_exists {
            pot.write_all(
                &format!(
                    r#"
{}msgid {}
msgid_plural {}
msgstr[0] ""
"#,
                    code_path,
                    message,
                    pl
                )
                .into_bytes(),
            )
            .expect("Couldn't write message to .pot (plural)");
        }
        let count = format_args
            .clone()
            .into_iter()
            .next()
            .expect("Item count should be specified")
            .clone();
        res.extend(quote!(
            .ngettext($message, $pl, $count as u64)
        ))
    } else {
        if !already_exists {
            pot.write_all(
                &format!(
                    r#"
{}msgid {}
msgstr ""
"#,
                    code_path,
                    message
                )
                .into_bytes(),
            )
            .expect("Couldn't write message to .pot");
        }

        res.extend(quote!(
            .gettext($message)
        ))
    }
    let mut args = vec![];
    let mut first = true;
    for arg in format_args {
        if first {
            first = false;
        } else {
            args.push(TokenTree::Punct(Punct::new(',', Spacing::Alone)));
        }
        args.extend(quote!(Box::new($arg)));
    }
    let mut fargs = TokenStream::new();
    fargs.extend(args);
    let res = quote!({
        ::gettext_utils::try_format($res, &[$fargs]).expect("Error while formatting message")
    });
    res
}

/// Gets the domain set in `gettext::compile_i18n`.
///
/// You can also use `env!("GETTEXT_MACROS_DOMAIN")`.
///
/// ## Note
///
/// This macro will panic if `gettext_utils::compile_i18n` hasn't been called
/// in the build script.
#[proc_macro]
pub fn i18n_domain(_: TokenStream) -> TokenStream {
    let domain = get_current_domain();
    let tok = TokenTree::Literal(Literal::string(&domain));
    quote!($tok)
}

/// Use this macro to staticaly import translations into your final binary.
///
/// ## Example
///
/// ```rust,ignore
/// # //ignore because there is no translation file provided with rocket_i18n
/// # #[macro_use]
/// # extern crate rocket_i18n;
/// # use rocket_i18n::Translations;
/// let tr: Translations = include_i18n!();
/// ```
///
/// ## Note
///
/// This macro will panic if `gettext_utils::compile_i18n` hasn't been called
/// in the build script.
#[proc_macro]
pub fn include_i18n(_: TokenStream) -> TokenStream {
    let domain = get_current_domain();
    let file = fs::read_to_string(utils::domain_path(&domain)).unwrap();
    let locales = file.lines()
        .map(|l| {
            let mo_bytes = utils::compile_domain_lang(&domain, l);
            let lang = TokenTree::Literal(Literal::string(&l));
            let mo_bytes = TokenTree::Literal(Literal::byte_string(&mo_bytes));
            quote! {
                ($lang, ::gettext::Catalog::parse(&$mo_bytes[..])
                    .expect("Error while loading catalog")),
            }
        }).collect::<TokenStream>();

    quote!({
        vec![
            $locales
        ]
    })
}

fn get_current_domain() -> String {
    env::var("GETTEXT_MACROS_DOMAIN")
        .expect("You need to call gettext_utils::compile_i18n in build.rs")
}
