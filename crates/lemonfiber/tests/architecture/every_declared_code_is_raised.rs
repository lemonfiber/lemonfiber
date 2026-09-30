//! Every code the registry declares is named by code a release is built from.
//!
//! The reference and the operators' page are generated from the registry and both say
//! they list every code lemonfiber can raise and nothing else. A declaration whose last
//! raiser was taken away goes on publishing a code no run can produce, and the page goes
//! on explaining a code no operator will ever meet.
//!
//! A code counts as raised where shipped code names it as `<family>::<NAME>`, written out
//! or brought in with `use`. Named rather than traced to a `Problem`, because codes also
//! travel through tables and helpers before a problem carries them, and a name in shipped
//! code is the reading that needs no opinion about how it is used.

use std::collections::BTreeSet;

use lemonfiber_core::error::codes::{declared, every, Declared};

use crate::source_tree::{crates_that_ship, in_a_crate_that_ships, shipped};

/// A code's family and the name it is declared under: `("quota", "NO_LIMIT")`.
type Reach = (String, String);

/// Every declared code is named somewhere a release is built from.
///
/// The cure for a red run is not a reference kept alive for this rule. A code nothing
/// raises leaves the registry, and its number goes into `codes::RETIRED`, so it is never
/// declared again with a different meaning.
#[test]
fn every_declared_code_is_named_by_code_that_ships() {
    let crates = crates_that_ship();
    let named: BTreeSet<Reach> = shipped()
        .into_iter()
        .filter(|(file, _)| in_a_crate_that_ships(file, &crates))
        .flat_map(|(_, ships)| reaches(&ships))
        .collect();
    let codes = every();
    let unraised: Vec<String> = codes
        .iter()
        .filter(|code| !declared(**code).is_some_and(|one| named.contains(&reach(one))))
        .map(ToString::to_string)
        .collect();
    assert!(
        unraised.len() < codes.len(),
        "no declared code is named anywhere that ships, which means this is reading the \
         names wrong and would pass over a code nothing raises"
    );
    assert!(
        unraised.is_empty(),
        "declared and raised by nothing a release is built from — take each out of the \
         registry and put its number in `codes::RETIRED`: {unraised:?}"
    );
}

/// Both shapes a code is named in are read: a path, and a `use` group.
#[test]
fn a_code_is_found_written_out_and_brought_in_by_a_group() {
    let found = reaches(
        "use crate::error::codes::quota::{NOBODY, NO_LIMIT};\n\
         let code = codes::plugin::NOT_CATALOGUED;\n\
         // quota::NOT_WAITING is a comment, and names nothing\n\
         let said = \"quota::TOO_SOON\";",
    );
    let expected: BTreeSet<Reach> = [
        ("quota", "NOBODY"),
        ("quota", "NO_LIMIT"),
        ("codes", "plugin"),
        ("plugin", "NOT_CATALOGUED"),
        ("codes", "quota"),
        ("error", "codes"),
        ("crate", "error"),
    ]
    .into_iter()
    .map(|(family, name)| (family.to_owned(), name.to_owned()))
    .collect();
    assert_eq!(found, expected);
}

/// The family and name a declared code is reached by.
///
/// A family's module is its prefix in lower case, which is how the registry declares
/// every one of them.
fn reach(one: Declared) -> Reach {
    let code = one.code();
    let family = code
        .as_str()
        .rsplit_once('-')
        .map_or("", |(family, _)| family);
    (family.to_lowercase(), one.name().to_owned())
}

/// Every `a::B` a file writes, and every `a::{B, C}` it groups, as `(a, B)` pairs.
///
/// Read through rustc's own lexer, so a name in a comment or a string is not a name.
fn reaches(text: &str) -> BTreeSet<Reach> {
    let tokens = code_tokens(text);
    let mut found = BTreeSet::new();
    for (at, window) in tokens.windows(4).enumerate() {
        let [(true, family), (false, ":"), (false, ":"), next] = window else {
            continue;
        };
        match next {
            (true, name) => {
                found.insert(((*family).to_owned(), (*name).to_owned()));
            }
            (false, "{") => {
                let group = tokens.get(at + 4..).unwrap_or_default();
                for name in grouped(group) {
                    found.insert(((*family).to_owned(), name.to_owned()));
                }
            }
            (false, _) => {}
        }
    }
    found
}

/// The identifiers inside a group, up to the brace that closes it.
fn grouped<'text>(tokens: &[(bool, &'text str)]) -> Vec<&'text str> {
    let mut depth = 1_usize;
    let mut names = Vec::new();
    for (ident, said) in tokens {
        match (ident, *said) {
            (false, "{") => depth += 1,
            (false, "}") => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            (true, name) => names.push(name),
            (false, _) => {}
        }
    }
    names
}

/// A file's tokens, without whitespace or comments, each marked by whether it is a name.
fn code_tokens(text: &str) -> Vec<(bool, &str)> {
    let mut tokens = Vec::new();
    let mut at = 0;
    for token in rustc_lexer::tokenize(text, rustc_lexer::FrontmatterAllowed::No) {
        let length = token.len as usize;
        let said = text.get(at..at + length).unwrap_or_default();
        at += length;
        match token.kind {
            rustc_lexer::TokenKind::Whitespace
            | rustc_lexer::TokenKind::LineComment { .. }
            | rustc_lexer::TokenKind::BlockComment { .. } => {}
            rustc_lexer::TokenKind::Ident => tokens.push((true, said)),
            _ => tokens.push((false, said)),
        }
    }
    tokens
}
