use std::{env, fs, io, process};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Initializes the translation environment.
///
/// This function must be called in a build script (e.g. `build.rs`) before you
/// can use `i18n!` and `include_i18n!`.
///
/// This function sets the build environment variable `GETTEXT_MACROS_DOMAIN` to
/// the value of the given `domain`. The variable is accessible in your crate
/// with `env!("GETTEXT_MACROS_DOMAIN")`.
///
/// ## Example
///
/// In `build.rs`:
///
/// ```rust,ignore
/// extern crate gettext_utils;
///
/// fn main() {
///     gettext_utils::compile_i18n("my_app", &["en", "fr", "de"]);
/// }
/// ```
pub fn compile_i18n(domain: &str, langs: &[&str]) {
    write_domain_file(domain, langs)
        .expect("Failed to create the domain file");

    let translations_source = translations_source_path(domain);
    if !translations_source.is_dir() {
        fs::create_dir_all(&translations_source)
            .expect("Failed to create po/{domain}/");
    }

    println!("cargo:rustc-env=GETTEXT_MACROS_DOMAIN={}", domain);
    for lang in langs {
        let po = po_path(domain, lang);
        println!("cargo:rerun-if-changed={}", po.to_str().unwrap());
    }
}

/// Creates a file with the list of languages, located at
/// `target/translations/.domains/domain`.
///
/// The languages are written line by line.
///
/// This file is used by `include_i18n!` to read the right `.mo` files in
/// `target/translations/{lang}/{current domain}`.
fn write_domain_file(domain: &str, langs: &[&str]) -> io::Result<()> {
    let domain_path = domain_path(domain);
    fs::create_dir_all(domain_path.parent().unwrap())?;
    let mut domain_file = fs::File::create(domain_path)?;
    for lang in langs {
        domain_file.write_all(lang.as_bytes())?;
        domain_file.write_all(b"\n")?;
    }
    Ok(())
}

#[doc(hidden)]
/// Merges the .pot with the .po file return the compiled `.mo` contents.
///
/// Called by `include_i18n!`.
pub fn compile_domain_lang(domain: &str, lang: &str) -> Vec<u8> {
    let po = po_path(domain, lang);
    let pot = pot_path(domain);

    merge_po_with_pot(&po, &pot, lang).expect("Failed to update the .po files");
    compile_po(&po).expect("Failed to compile the .po files")
}

#[doc(hidden)]
/// Creates and initializes the `.pot` file for the given `domain`.
pub fn init_pot(pot: &Path, domain: &str) -> io::Result<()> {
    let base_pot = format!(r#"msgid ""
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
"Plural-Forms: nplurals=2; plural=(n != 1);\n"
"#, domain);
    let mut pot_file = fs::File::create(pot)?;
    pot_file.write_all(base_pot.as_bytes())?;
    Ok(())
}

/// Put the new strings from the given `.pot` file into the given `.po` file.
///
/// The `.pot` file must exist before calling this function (or it will return
/// an error).
fn merge_po_with_pot(po: &Path, pot: &Path, lang: &str) -> io::Result<()> {
    let po_file_exists = po.is_file();
    let po_path = po.to_str().unwrap();
    let pot_path = pot.to_str().unwrap();
    if po_file_exists {
        println!("    Updating {}", po_path);
        let status = process::Command::new("msgmerge")
            .arg("--verbose")
            .arg("--update")
            .arg(po_path)
            .arg(pot_path)
            .stdout(process::Stdio::null())
            .status()?;
        if !status.success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "msgmerge returned with a non-zero status"));
        }
    } else {
        println!("    Creating {}", po_path);
        let status = process::Command::new("msginit")
            .arg("--input")
            .arg(pot_path)
            .arg("--output-file")
            .arg(po_path)
            .arg("--locale")
            .arg(lang)
            .arg("--no-translator")
            .stdout(process::Stdio::null())
            .status()?;
        if !status.success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "msginit returned with a non-zero status"));
        }
    }
    Ok(())
}

/// Compile the given `.po` file.
fn compile_po(po: &Path) -> io::Result<Vec<u8>> {
    let po_path = po.to_str().unwrap();
    println!("   Compiling {}", po_path);
    let output = process::Command::new("msgfmt")
        .arg("--output-file")
        .arg("-")
        .arg(po_path)
        .output()?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "msgfmt returned with a non-zero status"))
    }
}

#[doc(hidden)]
/// `target/translations/.domains/{domain}`
///
/// This file contains the languages enabled in the domain.
pub fn domain_path(domain: &str) -> PathBuf {
    translations_target_path()
        .join(".domains")
        .join(domain)
}

/// `target/translations`
///
/// This folder contains the translation artifacts (mostly the `.mo` files).
fn translations_target_path() -> PathBuf {
    env_path("CARGO_TARGET_DIR")
        .unwrap_or_else(|| crate_path().join("target"))
        .join("translations")
}

/// `po/{domain}/{lang}.po`
///
/// Translation sources for the given domain and language.
fn po_path(domain: &str, lang: &str) -> PathBuf {
    let lang_file = format!("{}.po", lang);
    translations_source_path(domain).join(lang_file)
}

#[doc(hidden)]
/// `target/{domain}/{domain}.pot`
///
/// Translation model for the given domain.
pub fn pot_path(domain: &str) -> PathBuf {
    let pot_file = format!("{}.pot", domain);
    translations_source_path(domain).join(pot_file)
}

/// `po/{domain}`
///
/// Translation sources for the given domain.
fn translations_source_path(domain: &str) -> PathBuf {
    crate_path().join("po").join(domain)
}

/// The crate root.
fn crate_path() -> PathBuf {
    let this_crate_root = env_path("CARGO_MANIFEST_DIR").unwrap();
    let is_from_workspace = this_crate_root.parent()
        .map(|parent| parent.join("Cargo.toml").exists())
        .unwrap_or(false);
    if is_from_workspace {
        this_crate_root.parent().unwrap().to_path_buf()
    } else {
        this_crate_root
    }
}

/// Shortcut to get a `PathBuf` from an environment variable that contains
/// a path.
fn env_path(var: &str) -> Option<PathBuf> {
    env::var(var).ok().map(PathBuf::from)
}

#[derive(Debug)]
#[doc(hidden)]
pub enum FormatError {
    UnmatchedCurlyBracket,
    InvalidPositionalArgument,
}

#[doc(hidden)]
pub fn try_format<'a>(
    str_pattern: &'a str,
    argv: &[::std::boxed::Box<dyn ::std::fmt::Display + 'a>],
) -> ::std::result::Result<::std::string::String, FormatError> {
    use ::std::fmt::Write;
    use ::std::iter::Iterator;

    //first we parse the pattern
    let mut pattern = vec![];
    let mut vars = vec![];
    let mut finish_or_fail = false;
    for (i, part) in str_pattern.split('}').enumerate() {
        if finish_or_fail {
            return ::std::result::Result::Err(FormatError::UnmatchedCurlyBracket);
        }
        if part.contains('{') {
            let mut part = part.split('{');
            let text = part.next().unwrap();
            let arg = part.next().ok_or(FormatError::UnmatchedCurlyBracket)?;
            if part.next() != ::std::option::Option::None {
                return ::std::result::Result::Err(FormatError::UnmatchedCurlyBracket);
            }
            pattern.push(text);
            vars.push(
                argv.get::<usize>(if arg.len() > 0 {
                    arg.parse()
                        .map_err(|_| FormatError::InvalidPositionalArgument)?
                } else {
                    i
                })
                .ok_or(FormatError::InvalidPositionalArgument)?,
            );
        } else {
            finish_or_fail = true;
            pattern.push(part);
        }
    }

    //then we generate the result String
    let mut res = ::std::string::String::with_capacity(str_pattern.len());
    let mut pattern = pattern.iter();
    let mut vars = vars.iter();
    while let ::std::option::Option::Some(text) = pattern.next() {
        res.write_str(text).unwrap();
        if let ::std::option::Option::Some(var) = vars.next() {
            res.write_str(&format!("{}", var)).unwrap();
        }
    }
    ::std::result::Result::Ok(res)
}
