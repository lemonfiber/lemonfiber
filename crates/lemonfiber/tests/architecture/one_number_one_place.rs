//! A number that says it belongs to somewhere else must be read from there.
//!
//! Three numbers in this workspace were written out twice, and all three said so
//! in their own documentation: one called itself *the same number the command
//! line begins with*, one *the terminal dashboard's own tick*, one *the budget a
//! real start is given, so what these read is what an operator reads*. Every
//! sentence was true on the day it was written and nothing was keeping any of
//! them — tune either copy and the comment goes on reading true while the two
//! answer the same question differently.
//!
//! That is worse than no comment at all. An unremarked literal gets checked by
//! whoever changes it; one that announces its own provenance gets believed.
//!
//! So the rule is not *never write a number twice*. Plenty of numbers land on the
//! same value for unrelated reasons — a retry budget and a repair budget are both
//! three, for arguments that have nothing to do with each other — and forcing
//! those to share would couple things that ought to move apart.
//!
//! So this is narrower, and refuses one thing: a comment that says a number is
//! another of our surfaces' and then writes the number out. A rule on names alone
//! was tried and thrown away — it found sixteen constants sharing a name across
//! unrelated modules and two that mattered, and a rule that is wrong seven times
//! out of eight is a rule somebody deletes rather than obeys.

use std::path::Path;

use crate::source_tree::sources;

/// This product's own surfaces, as a comment refers to them.
///
/// A closed list, because the claim being refused is *parity between two of our
/// surfaces* and nothing wider. A number documented as belonging to somebody
/// else's API — `TMDB_SEARCH` is seventeen in the service's own numbering — is
/// making a true statement about a thing this workspace cannot read, and there is
/// no edit that would improve it.
const OUR_SURFACES: &[&str] = &[
    "the command line",
    "the terminal dashboard",
    "the dashboard",
    "the web app",
    "the browser",
    "the http surface",
    "the api",
];

/// Phrases that assert a value is the same one as somewhere else's.
///
/// Sameness of a *value*, not of a reason: "the same number", never "the same
/// way". A rule that fires on ordinary prose is a rule somebody turns off, and
/// this codebase's comments are mostly ordinary prose.
const ASSERTS_SAMENESS: &[&str] = &[
    "the same number",
    "the same length",
    "the same value",
    "the same cadence",
    "own tick",
    "own number",
    "own budget",
];

/// A constant that claims one of our other surfaces' number reads it from there.
///
/// The cure is one of two edits and the author picks which. Point the constant at
/// the place it names, and the sentence is held by the compiler. Or cut the claim
/// and justify the number where it stands, which is honest and leaves the next
/// reader checking it rather than trusting it. What this refuses is the third
/// option, the one that kept happening: a sentence doing a reference's work.
#[test]
fn a_constant_claiming_another_surfaces_number_reads_it_from_there() {
    let mut claimed = Vec::new();

    for (path, text) in sources() {
        if names_the_rule_itself(&path) {
            continue;
        }
        let lines: Vec<&str> = text.lines().collect();
        for (at, line) in lines.iter().enumerate() {
            let Some((_, value)) = declared(line) else {
                continue;
            };
            // A digit anywhere in the value is the number being written out here —
            // which is the test that matters, and the one a look for "is this a
            // literal" fails: `Duration::from_secs(1)` names a path and a call and
            // is still a number somebody typed.
            if !value.chars().any(|each| each.is_ascii_digit()) {
                continue;
            }
            let said = documentation_above(&lines, at);
            let (Some(surface), Some(sameness)) = (
                OUR_SURFACES.iter().find(|each| said.contains(**each)),
                ASSERTS_SAMENESS.iter().find(|each| said.contains(**each)),
            ) else {
                continue;
            };
            claimed.push(format!(
                "{}:{} says \"{sameness}\" of {surface} and then writes the number \
                 out: {}",
                path.display(),
                at + 1,
                line.trim()
            ));
        }
    }

    assert!(
        claimed.is_empty(),
        "a comment is doing a reference's work — point the constant at what it names, \
         or stop naming it:\n{}",
        claimed.join("\n")
    );
}

/// Whether this file is the one holding the phrases above.
///
/// It quotes every phrase it looks for, so without this the rule reports itself —
/// which is true and useless.
fn names_the_rule_itself(path: &Path) -> bool {
    path.ends_with("one_number_one_place.rs")
}

/// The name and value of a constant declared on this line, if it is one.
fn declared(line: &str) -> Option<(&str, &str)> {
    let rest = line.trim_start();
    let rest = rest.strip_prefix("pub ").unwrap_or(rest);
    let rest = rest.strip_prefix("const ")?;
    let (name, rest) = rest.split_once(':')?;
    let (_, value) = rest.split_once('=')?;
    Some((name.trim(), value.trim().trim_end_matches(';').trim()))
}

/// The doc comment directly above this line, lowercased and run together.
///
/// Read upward through the unbroken run of `///` lines, so a claim made in the
/// first sentence is found from a declaration several lines below it — which is
/// where these sentences live, since the reason a number is what it is takes a
/// paragraph and the number is one line.
fn documentation_above(lines: &[&str], at: usize) -> String {
    let mut doc = Vec::new();
    // `get` rather than a range index: the workspace denies slicing, and a caller
    // handing an index past the end would take the whole test binary down with it
    // rather than reporting a claim it could not read.
    for above in lines.get(..at).unwrap_or_default().iter().rev() {
        let trimmed = above.trim_start();
        if trimmed.starts_with("///") {
            doc.push(trimmed.trim_start_matches('/').trim().to_lowercase());
        } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
            break;
        }
    }
    doc.join(" ")
}
