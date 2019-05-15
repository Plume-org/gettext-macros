#![feature(proc_macro_hygiene, proc_macro_quote, proc_macro_span, uniform_paths)]

extern crate proc_macro;
use proc_macro::{Delimiter, Literal, Spacing, Punct, TokenStream, TokenTree, quote, token_stream::IntoIter as TokenIter};
use std::{
    env,
    fs::{create_dir_all, read, File, OpenOptions},
    io::{BufRead, Read, Seek, SeekFrom, Write},
    iter::FromIterator,
    path::Path,
    process::{Command, Stdio},
};

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

fn named_arg(mut input: TokenIter, name: &'static str) -> Option<TokenStream> {
    input.next().and_then(|t| match t {
        TokenTree::Ident(ref i) if i.to_string() == name => {
            input.next(); // skip "="
            Some(input.take_while(|tok| match tok {
                TokenTree::Punct(_) => false,
                _ => true,
            }).collect())
        },
        _ => None,
    })
}

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
    let out_dir = Path::new(&env::var("CARGO_TARGET_DIR").unwrap_or("target/debug".into()))
        .join("gettext_macros");
    let domain = read(out_dir.join(env::var("CARGO_PKG_NAME").expect("Please build with cargo")))
        .expect("Coudln't read domain, make sure to call init_i18n! before")
        .lines()
        .next()
        .expect("Invalid config file. Make sure to call init_i18n! before this macro")
        .expect("IO error while reading config");
    let mut pot = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(format!("po/{0}/{0}.pot", domain))
        .expect("Couldn't open .pot file");

    for _ in 0..(catalog.len() + 1) {
        input.next();
    }
    let context = named_arg(input.clone(), "context");
    if let Some(c) = context.clone() {
        for _ in 0..(c.into_iter().count() + 3) {
            input.next();
        }
    }
    let message = trim(input.next().expect("Expected a message to translate"));

    let mut contents = String::new();
    pot.read_to_string(&mut contents).expect("IO error while reading .pot file");
    pot.seek(SeekFrom::End(0)).expect("IO error while seeking .pot file to end");

    let already_exists = is_empty(&message) || contents.contains(&format!("{}msgid {}", context.clone().map(|c| format!("msgctxt {}\n", c)).unwrap_or_default(), message));

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
    let code_path = match file.to_str() {
        Some(path) if !file.is_absolute() => format!("#: {}:{}\n", path, line),
        _ => String::new(),
    };
    let prefix = if let Some(c) = context.clone() {
        format!("{}msgctxt {}\n", code_path, c)
    } else {
        code_path
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
                    prefix,
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
        if let Some(c) = context {
            res.extend(quote!(
                .npgettext($c, $message, $pl, $count as u64)
            ))
        } else {
            res.extend(quote!(
                .ngettext($message, $pl, $count as u64)
            ))
        }
    } else {
        if !already_exists {
            pot.write_all(
                &format!(
                    r#"
{}msgid {}
msgstr ""
"#,
                    prefix,
                    message
                )
                .into_bytes(),
            )
            .expect("Couldn't write message to .pot");
        }
        if let Some(c) = context {
            res.extend(quote!(
                .pgettext($c, $message)
            ))
        } else {
            res.extend(quote!(
                .gettext($message)
            ))
        }
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
        use runtime_fmt::*;
        rt_format!($res, $fargs).expect("Error while formatting message")
    });
    res
}

#[proc_macro]
pub fn init_i18n(input: TokenStream) -> TokenStream {
    let mut input = input.into_iter();
    let domain = match input.next() {
        Some(TokenTree::Literal(lit)) => lit.to_string().replace("\"", ""),
        Some(_) => panic!("Domain should be a str"),
        None => panic!("Expected a translation domain (for instance \"myapp\")"),
    };
    let mut langs = vec![];
    if let Some(t) = input.next() {
        if is(&t, ',') {
            match input.next() {
                Some(TokenTree::Ident(i)) => {
                    langs.push(i);
                    loop {
                        let next = input.next();
                        if next.is_none() || !is(&next.expect("Unreachable: next should be Some"), ',') {
                            break;
                        }
                        match input.next() {
                            Some(TokenTree::Ident(i)) => {
                                langs.push(i);
                            }
                            _ => panic!("Expected a language identifier"),
                        }
                    }
                }
                _ => panic!("Expected a language identifier"),
            }
        } else {
            panic!("Expected  `,`")
        }
    }

    // emit file to include
    let out_dir = Path::new(&env::var("CARGO_TARGET_DIR").unwrap_or("target/debug".into()))
        .join("gettext_macros");
    let out = out_dir.join(env::var("CARGO_PKG_NAME").expect("Please build with cargo"));
    create_dir_all(out_dir).expect("Couldn't create output dir");
    let mut out = File::create(out).expect("Metadata file couldn't be open");
    writeln!(out, "{}", domain).expect("Couldn't write domain");
    for l in langs {
        writeln!(out, "{}", l).expect("Couldn't write lang");
    }

    // write base .pot
    create_dir_all(format!("po/{}", domain)).expect("Couldn't create po dir");
    let mut pot = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(format!("po/{0}/{0}.pot", domain))
        .expect("Couldn't open .pot file");
    pot.write_all(
        &format!(
            r#"msgid ""
msgstr ""
"Project-Id-Version: {}\n"
"Report-Msgid-Bugs-To: \n"
"POT-Creation-Date: 2018-06-15 16:33-0700\n"
"PO-Revision-Date: YEAR-MO-DA HO:MI+ZONE\n"
"Last-Translator: FULL NAME <EMAIL@ADDRESS>\n"
"Language-Team: LANGUAGE <LL@li.org>\n"
"Language: \n"
"MIME-Version: 1.0\n"
"Content-Type: text/plain; charset=UTF-8\n"
"Content-Transfer-Encoding: 8bit\n"
"Plural-Forms: nplurals=INTEGER; plural=EXPRESSION;\n"
"#,
            domain
        )
        .into_bytes(),
    )
    .expect("Couldn't init .pot file");

    quote!()
}

#[proc_macro]
pub fn i18n_domain(_: TokenStream) -> TokenStream {
    let out_dir = Path::new(&env::var("CARGO_TARGET_DIR").unwrap_or("target/debug".into()))
        .join("gettext_macros");
    let domain = read(out_dir.join(env::var("CARGO_PKG_NAME").expect("Please build with cargo")))
        .expect("Coudln't read domain, make sure to call init_i18n! before")
        .lines()
        .next()
        .expect("Invalid config file. Make sure to call init_i18n! before this macro")
        .expect("IO error while reading config");
    let tok = TokenTree::Literal(Literal::string(&domain));
    quote!($tok)
}

#[proc_macro]
pub fn compile_i18n(_: TokenStream) -> TokenStream {
    let out_dir = Path::new(&env::var("CARGO_TARGET_DIR").unwrap_or("target/debug".into()))
        .join("gettext_macros");
    let file = read(out_dir.join(env::var("CARGO_PKG_NAME").expect("Please build with cargo")))
        .expect("Coudln't read domain, make sure to call init_i18n! before");
    let mut lines = file.lines();
    let domain = lines.next()
        .expect("Invalid config file. Make sure to call init_i18n! before this macro")
        .expect("IO error while reading config");
    let locales = lines.map(|l| l.expect("IO error while reading locales from config")).collect::<Vec<_>>();

    let pot_path = root_crate_path().join("po")
        .join(domain.clone())
        .join(format!("{}.pot", domain));

    for lang in locales {
        let po_path = root_crate_path().join("po").join(domain.clone()).join(format!("{}.po", lang.clone()));
        if po_path.exists() && po_path.is_file() {
            // Update it
            Command::new("msgmerge")
                .arg("-U")
                .arg(po_path.to_str().expect("msgmerge: PO path error"))
                .arg(pot_path.to_str().expect("msgmerge: POT path error"))
                .stdout(Stdio::null())
                .status()
                .map(|s| {
                    if !s.success() {
                        panic!("Couldn't update PO file")
                    }
                })
                .expect("Couldn't update PO file. Make sure msgmerge is installed.");
        } else {
            println!("Creating {}", lang.clone());
            // Create it from the template
            Command::new("msginit")
                .arg(format!("--input={}", pot_path.to_str().expect("msginit: POT path error")))
                .arg(format!("--output-file={}", po_path.to_str().expect("msginit: PO path error")))
                .arg("-l")
                .arg(lang.clone())
                .arg("--no-translator")
                .stdout(Stdio::null())
                .status()
                .map(|s| {
                    if !s.success() {
                        panic!("Couldn't init PO file (gettext returned an error)")
                    }
                })
                .expect("Couldn't init PO file. Make sure msginit is installed.");
        }

        // Generate .mo
        let mo_dir = root_crate_path().join("translations")
            .join(lang.clone())
            .join("LC_MESSAGES");
        create_dir_all(mo_dir.clone()).expect("Couldn't create MO directory");
        let mo_path = mo_dir.join(format!("{}.mo", domain));

        Command::new("msgfmt")
            .arg(format!("--output-file={}", mo_path.to_str().expect("msgfmt: MO path error")))
            .arg(po_path)
            .stdout(Stdio::null())
            .status()
            .map(|s| {
                if !s.success() {
                    panic!("Couldn't compile translations (gettext returned an error)")
                }
            })
            .expect("Couldn't compile translations. Make sure msgfmt is installed");
    }
    quote!()
}

/// Use this macro to staticaly import translations into your final binary.
///
/// ```rust,ignore
/// # //ignore because there is no translation file provided with rocket_i18n
/// # #[macro_use]
/// # extern crate rocket_i18n;
/// # use rocket_i18n::Translations;
/// let tr: Translations = include_i18n!();
/// ```
#[proc_macro]
pub fn include_i18n(_: TokenStream) -> TokenStream {
    let out_dir = Path::new(&env::var("CARGO_TARGET_DIR").unwrap_or("target/debug".into()))
        .join("gettext_macros");
    let file = read(out_dir.join(env::var("CARGO_PKG_NAME").expect("Please build with cargo")))
        .expect("Coudln't read domain, make sure to call init_i18n! before");
    let mut lines = file.lines();
    let domain = lines.next()
        .expect("Invalid config file. Make sure to call init_i18n! before this macro")
        .expect("IO error while reading config");
    let locales = lines
		.map(|l| l.expect("IO error while reading locales from config"))
		.map(|l| {
            let lang = TokenTree::Literal(Literal::string(&l));
            let path = root_crate_path().join("translations").join(l).join("LC_MESSAGES").join(format!("{}.mo", domain));
            let path = TokenTree::Literal(Literal::string(path.to_str().expect("Couldn't write MO file path")));
            quote!{
                ($lang, ::gettext::Catalog::parse(
                    &include_bytes!(
                        $path
                    )[..]
                ).expect("Error while loading catalog")),
            }
		}).collect::<TokenStream>();

    quote!({
        vec![
            $locales
        ]
    })
}

fn root_crate_path() -> std::path::PathBuf {
    let path = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set. Please use cargo to compile your crate.");
    let path = Path::new(&path);
    if path.parent().expect("No parent dir").join("Cargo.toml").exists() {
        path.parent().expect("No parent dir").to_path_buf()
    } else {
        path.to_path_buf()
    }
}
