//! A surface reaches the core through `dispatch` and nothing else.
//!
//! The command line and the web API may name from `lemonfiber_core` the entry point
//! `app::dispatch`, its types, its constants and the functions of those types, and
//! nothing lowercase besides: no free function, and no module brought in to call one
//! through. A pure helper a surface needs lives in a crate of its own. Neither surface
//! names the context's `seams`: what it needs of the world beyond the core, a clock or
//! randomness, it holds itself.
//!
//! A plain check of names, read off the lexed source the compiler reads, with nothing
//! excused. The code a surface builds only for its tests is not the surface, and is
//! not read.
//!
//! The one command that wakes phones and tells back-ends of a new alert is the
//! operator's dashboard gather, and it is sent from one place: the dashboard source
//! the run serving the web surface gathers, built by that run alone.

use std::path::Path;

use crate::source_tree::{production, sources, test_only};

/// The source trees of the two surfaces.
const SURFACES: [&str; 2] = ["crates/lemonfiber/src", "crates/lemonfiber-api/src"];

/// The core, as a surface names it.
const CORE: &str = "lemonfiber_core";

/// The one function a surface may call in the core, by its path there.
const ENTRY: [&str; 2] = ["app", "dispatch"];

/// The one file that sends the operator's gather.
const GATHERS: &str = "crates/lemonfiber-api/src/events/dashboard.rs";

/// The one file that builds the source sending it: the run's own.
const BUILDS: &str = "crates/lemonfiber-api/src/events/extending.rs";

/// The context's field holding the core's ports, which no surface names.
const SEAMS: &str = "seams";

/// One lexed token of code, by the line it starts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token<'a> {
    /// An identifier or a keyword.
    Name(&'a str),
    /// `::`.
    Joint,
    /// Any other punctuation, by its text.
    Mark(&'a str),
    /// A literal or a lifetime, which names nothing.
    Literal,
}

/// The code tokens of `text`, each with its line, comments and whitespace left out.
fn tokens(text: &str) -> Vec<(usize, Token<'_>)> {
    let mut found: Vec<(usize, Token<'_>)> = Vec::new();
    let mut at = 0;
    let mut line = 1;
    for token in rustc_lexer::tokenize(text, rustc_lexer::FrontmatterAllowed::No) {
        let length = token.len as usize;
        let said = text.get(at..at + length).unwrap_or_default();
        let read = match token.kind {
            rustc_lexer::TokenKind::Whitespace
            | rustc_lexer::TokenKind::LineComment { .. }
            | rustc_lexer::TokenKind::BlockComment { .. } => None,
            rustc_lexer::TokenKind::Ident | rustc_lexer::TokenKind::RawIdent => {
                Some(Token::Name(said.trim_start_matches("r#")))
            }
            rustc_lexer::TokenKind::Literal { .. } | rustc_lexer::TokenKind::Lifetime { .. } => {
                Some(Token::Literal)
            }
            _ => Some(Token::Mark(said)),
        };
        if let Some(read) = read {
            if read == Token::Mark(":")
                && found
                    .last()
                    .is_some_and(|(_, last)| *last == Token::Mark(":"))
            {
                found.pop();
                found.push((line, Token::Joint));
            } else {
                found.push((line, read));
            }
        }
        line += said.matches('\n').count();
        at += length;
    }
    found
}

/// Whether a surface may name this path inside the core.
///
/// A type or a constant somewhere along it is what is named, and whatever follows one
/// is that type's own; with none, only the entry point.
fn may_name(path: &[&str]) -> bool {
    path.iter()
        .any(|segment| segment.starts_with(|first: char| first.is_ascii_uppercase()))
        || path == ENTRY
}

/// Every path into the core one `use` declares, read from just after `use`, with the
/// index just past its `;`.
fn declared<'a>(code: &[(usize, Token<'a>)], from: usize) -> (Vec<Vec<&'a str>>, usize) {
    let mut paths = Vec::new();
    let mut at = from;
    if code
        .get(at)
        .is_some_and(|(_, token)| *token == Token::Joint)
    {
        at += 1;
    }
    let rooted = code
        .get(at)
        .is_some_and(|(_, token)| *token == Token::Name(CORE));
    let end = tree(code, at, &[], rooted, &mut paths);
    let past = code
        .iter()
        .skip(end)
        .position(|(_, token)| *token == Token::Mark(";"))
        .map_or(code.len(), |semicolon| end + semicolon + 1);
    (paths, past)
}

/// One use tree from `at`, under `prefix`, gathering the paths into the core it ends
/// in; the index just past it.
fn tree<'a>(
    code: &[(usize, Token<'a>)],
    mut at: usize,
    prefix: &[&'a str],
    rooted: bool,
    paths: &mut Vec<Vec<&'a str>>,
) -> usize {
    let mut path: Vec<&'a str> = prefix.to_vec();
    loop {
        match code.get(at).map(|(_, token)| *token) {
            Some(Token::Name("self")) => {
                at += 1;
                break;
            }
            Some(Token::Name(name)) => {
                path.push(name);
                at += 1;
                if code
                    .get(at)
                    .is_some_and(|(_, token)| *token == Token::Joint)
                {
                    at += 1;
                    continue;
                }
                break;
            }
            Some(Token::Mark("*")) => {
                path.push("*");
                at += 1;
                break;
            }
            Some(Token::Mark("{")) => {
                at += 1;
                while code
                    .get(at)
                    .is_some_and(|(_, token)| *token != Token::Mark("}"))
                {
                    at = tree(code, at, &path, rooted, paths);
                    if code
                        .get(at)
                        .is_some_and(|(_, token)| *token == Token::Mark(","))
                    {
                        at += 1;
                    }
                }
                return at + 1;
            }
            _ => break,
        }
    }
    if code
        .get(at)
        .is_some_and(|(_, token)| *token == Token::Name("as"))
    {
        at += 2;
    }
    if rooted {
        paths.push(path.into_iter().skip(1).collect());
    }
    at
}

/// The path into the core written from `at`, which names the core; the index past it.
fn written<'a>(code: &[(usize, Token<'a>)], mut at: usize) -> (Vec<&'a str>, usize) {
    let mut path = Vec::new();
    at += 1;
    while code
        .get(at)
        .is_some_and(|(_, token)| *token == Token::Joint)
    {
        match code.get(at + 1).map(|(_, token)| *token) {
            Some(Token::Name(name)) => {
                path.push(name);
                at += 2;
            }
            _ => break,
        }
    }
    (path, at)
}

/// Every path into the core `text` names that a surface may not, by line.
fn unreachable_names(text: &str) -> Vec<(usize, String)> {
    let code = tokens(text);
    let mut refused = Vec::new();
    let mut at = 0;
    while let Some((line, token)) = code.get(at).copied() {
        if token == Token::Name("use") {
            let (paths, past) = declared(&code, at + 1);
            refused.extend(
                paths
                    .iter()
                    .filter(|path| !may_name(path))
                    .map(|path| (line, path.join("::"))),
            );
            at = past;
        } else if token == Token::Name(CORE) {
            let (path, past) = written(&code, at);
            if !may_name(&path) {
                refused.push((line, path.join("::")));
            }
            at = past;
        } else {
            at += 1;
        }
    }
    refused
}

/// Every line of `text` naming the seams.
fn seamed(text: &str) -> Vec<usize> {
    tokens(text)
        .into_iter()
        .filter(|(_, token)| *token == Token::Name(SEAMS))
        .map(|(line, _)| line)
        .collect()
}

/// The shipped text of every file of both surfaces, by path.
fn surfaces() -> Vec<(String, String)> {
    sources()
        .into_iter()
        .filter(|(path, _)| SURFACES.iter().any(|surface| path.starts_with(surface)))
        .filter(|(path, _)| !test_only(Path::new(path)))
        .map(|(path, text)| (path.display().to_string(), production(&text).to_owned()))
        .collect()
}

#[test]
fn a_surface_names_nothing_in_the_core_but_dispatch_its_types_and_its_constants() {
    let surfaces = surfaces();
    assert!(
        surfaces.len() > 100,
        "read {} files of the surfaces, so the check is looking in the wrong place",
        surfaces.len()
    );
    let refused: Vec<String> = surfaces
        .iter()
        .flat_map(|(path, text)| {
            unreachable_names(text)
                .into_iter()
                .map(move |(line, named)| format!("{path}:{line} names {CORE}::{named}"))
        })
        .collect();
    assert!(
        refused.is_empty(),
        "a surface reaches the core through `dispatch`, its types and its constants alone:\n{}",
        refused.join("\n")
    );
}

/// Every file of the surfaces naming `first` then `second`, joined by `::`.
fn naming(first: &str, second: &str) -> Vec<String> {
    surfaces()
        .into_iter()
        .filter(|(_, text)| {
            tokens(text).windows(3).any(|three| {
                three.first().map(|(_, token)| *token) == Some(Token::Name(first))
                    && three.get(1).map(|(_, token)| *token) == Some(Token::Joint)
                    && three.get(2).map(|(_, token)| *token) == Some(Token::Name(second))
            })
        })
        .map(|(path, _)| path)
        .collect()
}

#[test]
fn only_the_runs_own_dashboard_sends_the_gather_that_wakes_phones() {
    assert_eq!(naming("Surfacing", "Gather"), [GATHERS]);
    assert_eq!(naming("Dashboard", "against"), [BUILDS]);
}

#[test]
fn no_surface_names_the_seams() {
    let refused: Vec<String> = surfaces()
        .iter()
        .flat_map(|(path, text)| {
            seamed(text)
                .into_iter()
                .map(move |line| format!("{path}:{line}"))
        })
        .collect();
    assert!(
        refused.is_empty(),
        "a surface holds its own clock and randomness and names no `{SEAMS}`:\n{}",
        refused.join("\n")
    );
}

#[test]
fn the_check_sees_a_function_however_it_is_named_through_the_core() {
    let refused: Vec<String> = unreachable_names(
        "use lemonfiber_core::text::plain;\n\
         use lemonfiber_core::{app::{dispatch, Command}, model::kind};\n\
         use ::lemonfiber_core::app::*;\n\
         use lemonfiber_core as core;\n\
         use lemonfiber_core::wiring::{self, Unfilled as Gap};\n\
         fn shown() { lemonfiber_core::wishing::cover(&ctx); }\n",
    )
    .into_iter()
    .map(|(line, named)| format!("{line}:{named}"))
    .collect();
    assert_eq!(
        refused,
        [
            "1:text::plain",
            "2:model::kind",
            "3:app::*",
            "4:",
            "5:wiring",
            "6:wishing::cover"
        ]
    );
}

#[test]
fn the_check_lets_dispatch_types_constants_and_their_functions_through() {
    assert!(unreachable_names(
        "use lemonfiber_core::app::{dispatch, Command, Ctx as Context};\n\
         use lemonfiber_core::PRODUCT;\n\
         use lemonfiber_core::model::kind::{Kind, VERSION};\n\
         // lemonfiber_core::text::plain is named in a comment alone\n\
         fn asked() { lemonfiber_core::app::Command::polled(&x); \"lemonfiber_core::text\"; }\n",
    )
    .is_empty());
    assert_eq!(seamed("let x = ctx.seams.clock; // seams\n"), [1]);
}
