#![feature(proc_macro_hygiene, proc_macro_quote, proc_macro_span, uniform_paths)]

extern crate proc_macro;

use std::{env, io::{BufRead, Write}, fs::{create_dir_all, read, write, File, OpenOptions}, iter::FromIterator, path::{Path, PathBuf}};
use proc_macro::{Delimiter, Group, Literal, Spacing, Punct, TokenStream, TokenTree, quote};

fn is(t: &TokenTree, ch: char) -> bool {
    match t {
        TokenTree::Punct(p) => p.as_char() == ch,
        _ => false
    }
}

#[proc_macro]
pub fn i18n(input: TokenStream) -> TokenStream {
    let span = input.clone().into_iter().next().expect("Expected catalog").span();
    let mut input = input.into_iter();
    let catalog = input.clone().take_while(|t| !is(t, ',')).collect::<Vec<_>>();

    let file = span.source_file().path();
    let line = span.start().line;
    let mut domain = String::new();
    let out_dir = Path::new(&env::var("CARGO_TARGET_DIR").unwrap_or("target/debug".into())).join("gettext_macros");
    let domain_meta = read(out_dir.join("domain_paths")).expect("Domain metadata not found. Make sure to call configure_i18n! before using i18n!");
    let mut lines = domain_meta.lines();
    loop {
        domain = lines.next().unwrap().unwrap();
        if file.starts_with(lines.next().unwrap().unwrap()) {
            break;
        }
    }
    let out = read(out_dir.join(domain)).expect("Couldn't read output metadata");
    let pot_file = out.lines().next().unwrap().unwrap();
    let mut pot = OpenOptions::new().append(true).create(true).open(pot_file + ".pot").expect("Couldn't open .pot file");

    for _ in 0..(catalog.len() + 1) { input.next(); }
    let message = input.next().unwrap();

    let plural = match input.clone().next() {
        Some(t) => if is(&t, ',') {
            input.next();
            input.next()
        } else {
            None
        },
        _ => None
    };

    let mut format_args = vec![];
    println!("{}, input is {:?}", message.to_string(), input.clone().collect::<Vec<_>>());
    if let Some(TokenTree::Punct(p)) = input.next().clone() {
        if p.as_char() == ';' {
            loop {
                println!("looking for fa, {:?}", input.clone().collect::<Vec<_>>());
                let mut tokens = vec![];
                loop {
                    if let Some(t) = input.next().clone() {
                        if !is(&t, ',') {
                            println!("found tok");
                            tokens.push(t);
                        } else {
                            println!("next");
                            break;
                        }
                    } else {
                        println!("end");
                        break;
                    }
                }
                if tokens.is_empty() {
                    println!("tokens is empty");
                    break;
                }
                format_args.push(TokenStream::from_iter(tokens.into_iter()));
            }
        }
    }

    let mut res = TokenStream::from_iter(catalog);
    if let Some(pl) = plural {
        pot.write_all(&format!(r#"
# {}:{}
msgid {}
msgid_plural {}
msgstr ""
"#, file.to_str().unwrap(), line, message, pl).into_bytes()).expect("Couldn't write message to .pot (plural)");
        let count = format_args.clone().into_iter().next().expect("Item count should be specified").clone();
        res.extend(quote!(
            .ngettext($message, $pl, $count)
        ))
    } else {
        pot.write_all(&format!(r#"
# {}:{}
msgid {}
msgstr ""
"#, file.to_str().unwrap(), line, message).into_bytes()).expect("Couldn't write message to .pot");

        res.extend(quote!(
            .gettext($message)
        ))
    }
    let mut args = vec![];
    let mut first = true;
    println!("found {} args: {:?}", format_args.len(), format_args);
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
    println!("emit {}", res);
    res
}

#[proc_macro]
pub fn configure_i18n(input: TokenStream) -> TokenStream {
    let mut input = input.into_iter();
    let domain = match input.next() {
        Some(TokenTree::Literal(lit)) => lit.to_string().replace("\"", ""),
        Some(_) => panic!("Domain should be a str"),
        None => panic!("Expected a translation domain (for instance \"myapp\")"),
    };
    let mut langs = vec![];
    let mut path = String::new();
    if let Some(t) = input.next() {
        if is(&t, ',') {
            match input.next() {
                Some(TokenTree::Literal(l)) => { path = l.to_string().replace("\"", ""); }
                Some(TokenTree::Ident(i)) => {
                    langs.push(i);
                    loop {
                        let next = input.next();
                        if next.is_none() || !is(&next.unwrap(), ',') {
                            break;
                        }
                        match input.next() {
                            Some(TokenTree::Ident(i)) => { langs.push(i); },
                            _ => panic!("Expected a language identifier")
                        }
                    }
                },
                _ => panic!("Expected a language identifier or a path to store translations"),
            }
        } else {
            panic!("Expected  `,`")
        }
    };

    if let Some(t) = input.next() {
        if is(&t, ',') {
            match input.next() {
                Some(TokenTree::Ident(i)) => {
                    langs.push(i);
                    loop {
                        let next = input.next();
                        if next.is_none() || !is(&next.unwrap(), ',') {
                            break;
                        }
                        match input.next() {
                            Some(TokenTree::Ident(i)) => { langs.push(i); },
                            _ => panic!("Expected a language identifier")
                        }
                    }
                },
                _ => panic!("Expected a language identifier"),
            }
        } else {
            panic!("Expected  `,`")
        }
    };

    // emit file to include
    let out_dir = Path::new(&env::var("CARGO_TARGET_DIR").unwrap_or("target/debug".into())).join("gettext_macros");
    let out = out_dir.join(domain.clone());
    create_dir_all(out_dir).expect("Couldn't create output dir");
    let mut out = File::create(out).expect("Metadata file couldn't be open");
    writeln!(out, "{}", path).expect("Couldn't write path");
    for l in langs {
        writeln!(out, "{}", l).expect("Couldn't write lang");
    }

    // write base .pot
    create_dir_all(PathBuf::from(path.clone()).parent().unwrap()).expect("Couldn't create po dir");
    let mut pot = OpenOptions::new().write(true).create(true).truncate(true).open(path + ".pot").expect("Couldn't open .pot file");
    pot.write_all(&format!(r#"msgid ""
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
"#, domain).into_bytes()).expect("Couldn't init .pot file");
    quote!({})
}

#[proc_macro]
pub fn init_i18n(input: TokenStream) -> TokenStream {
    let domain_tok = input.into_iter().next().expect("Expected a domain");

    let out_dir = Path::new(&env::var("CARGO_TARGET_DIR").unwrap_or("target/debug".into())).join("gettext_macros");
    let code_file = domain_tok.span().source_file().path();
    let code_dir = code_file.parent().unwrap();
    let domain = domain_tok.to_string().replace("\"", "");
    write(
        out_dir.join("domain_paths"),
        String::from_utf8(read(out_dir.join("domain_paths")).unwrap_or_default()).unwrap() + &format!("{}\n{}\n", domain, code_dir.to_str().unwrap())
    ).expect("Couldn't update domain paths");
    let out = out_dir.join(domain.to_string().replace("\"", ""));
    let meta = read(out).expect("Couldn't read metadata file");
    let mut lines = meta.lines();
    let dir = TokenTree::Literal(Literal::string(&lines.next().expect("Metadata file is not properly configured")
        .expect("Couldn't read output dir location")));

    let mut langs = vec![];
    for lang in lines {
        langs.push(TokenTree::Literal(Literal::string(&lang.expect("Couldn't read lang"))));
    }
    let langs = TokenTree::Group(Group::new(
        Delimiter::Bracket,
        TokenStream::from_iter(langs.into_iter().map(|l| vec![l, TokenTree::Punct(Punct::new(',', Spacing::Alone))]).flatten()),
    ));
    quote!(
        pub mod __i18n {
            pub static DOMAIN: &'static str = $domain_tok;

            pub static PO_DIR: &'static str = $dir;

            pub fn langs() -> ::std::vec::Vec<&'static str> {
                vec!$langs
            }
        }
    )
}
