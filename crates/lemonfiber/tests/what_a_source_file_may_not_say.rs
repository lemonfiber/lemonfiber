//! Three things a source file may not contain, swept for as text.
//!
//! One subject under three spellings: the source says what it does, and does not
//! say what it is for or argue its way out of a standard. A suppression turns a
//! rule the whole tree is held to into one that applies wherever nobody objected. A
//! requirement identifier in a comment is a citation that rots — the spec moves,
//! the comment does not, and the reader is left trusting a number rather than the
//! sentence beside it.
//!
//! Text rather than structure, because all three are about what may be written at
//! all, and something written is exactly what a scan of the written thing can see.

mod source_tree;

use source_tree::sources;

/// Suppressions are not how a rule gets satisfied.
///
/// An allow attribute turns a standard the whole codebase is held to into one
/// that applies wherever nobody objected. Change the code, or change the rule
/// for everyone in the workspace manifest.
#[test]
fn no_lint_is_suppressed_in_source() {
    for (path, text) in sources() {
        if path.starts_with("crates") && path.to_string_lossy().contains("tests") {
            continue;
        }
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            assert!(
                !(trimmed.starts_with("#[allow(") || trimmed.starts_with("#![allow(")),
                "{}:{} suppresses a lint — change the code, or change the rule for everyone",
                path.display(),
                number + 1
            );
        }
    }
}
/// Requirement identifiers do not belong in code.
///
/// Provenance in a comment is worthless to the next reader and rots the moment
/// the requirement is superseded. Identifiers go in commits and pull requests;
/// code links to `.docs/`, and those pages cite the specification.
#[test]
fn no_requirement_identifier_appears_in_a_comment() {
    let prefixes = ["ARCH-R", "REPO-R", "GOV-R", "OPS-R", "Q-R", "DES-R", "ADR-"];

    for (path, text) in sources() {
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if !(trimmed.starts_with("//") || trimmed.starts_with("/*")) {
                continue;
            }
            for prefix in prefixes {
                assert!(
                    !line.contains(prefix),
                    "{}:{} cites `{prefix}…` in a comment — cite it in the commit instead",
                    path.display(),
                    number + 1
                );
            }
        }
    }
}
/// Feature-area identifiers are caught too.
///
/// Separate from the prefix list above because these have no fixed prefix — the
/// shape is an uppercase letter, a digit, a dash, an R, then a digit. Written as
/// a character test rather than as an example, since an example would be a
/// requirement identifier in a comment and this test would find it.
#[test]
fn no_feature_requirement_identifier_appears_in_a_comment() {
    for (path, text) in sources() {
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if !(trimmed.starts_with("//") || trimmed.starts_with("/*")) {
                continue;
            }
            let characters: Vec<char> = line.chars().collect();
            for window in characters.windows(5) {
                let [area, feature, dash, marker, index] = window else {
                    continue;
                };
                let looks_like_an_identifier = area.is_ascii_uppercase()
                    && feature.is_ascii_digit()
                    && *dash == '-'
                    && *marker == 'R'
                    && index.is_ascii_digit();
                assert!(
                    !looks_like_an_identifier,
                    "{}:{} cites a requirement in a comment — cite it in the commit instead",
                    path.display(),
                    number + 1
                );
            }
        }
    }
}
