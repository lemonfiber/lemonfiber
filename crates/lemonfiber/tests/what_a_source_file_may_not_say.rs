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
/// shape is an uppercase letter, a feature number, a dash, an R, then a digit.
/// Written as a character test rather than as an example, since an example would
/// be a requirement identifier in a comment and this test would find it.
#[test]
fn no_feature_requirement_identifier_appears_in_a_comment() {
    for (path, text) in sources() {
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if !(trimmed.starts_with("//") || trimmed.starts_with("/*")) {
                continue;
            }
            assert!(
                !cites_a_feature_requirement(line),
                "{}:{} cites a requirement in a comment — cite it in the commit instead",
                path.display(),
                number + 1
            );
        }
    }
}

/// Whether a line writes a feature-area requirement identifier.
///
/// A scan rather than a window of fixed width. A window of five characters can
/// only ever see a feature number of one digit, and the specification defines
/// four areas whose number is two — every identifier in those was invisible to a
/// rule that names itself after catching them. The feature number is read as one
/// or more digits for that reason, and nothing else about the shape moves.
fn cites_a_feature_requirement(line: &str) -> bool {
    let characters: Vec<char> = line.chars().collect();
    for (at, letter) in characters.iter().enumerate() {
        if !letter.is_ascii_uppercase() {
            continue;
        }
        let mut after = at + 1;
        while characters.get(after).is_some_and(char::is_ascii_digit) {
            after += 1;
        }
        if after == at + 1 {
            continue;
        }
        let reads = [
            characters.get(after),
            characters.get(after + 1),
            characters.get(after + 2),
        ];
        if let [Some('-'), Some('R'), Some(index)] = reads {
            if index.is_ascii_digit() {
                return true;
            }
        }
    }
    false
}

/// The scan is held to the shapes it exists to catch, and to those it must not.
///
/// Every subject is assembled from characters rather than written out, because a
/// literal one would be an identifier in this file and the sweep above reads this
/// file. What is worth pinning is the two-digit feature number: a rule that
/// catches the shorter shape and not the longer one goes green over exactly the
/// citations nobody notices.
#[test]
fn the_scan_reads_a_feature_number_of_any_length() {
    let identifier =
        |area: &str, feature: u32, index: u32| format!("// see {area}{feature}-R{index}");
    for (area, feature) in [("F", 1u32), ("F", 10), ("B", 10), ("D", 10), ("F", 11)] {
        let line = identifier(area, feature, 4);
        assert!(
            cites_a_feature_requirement(&line),
            "{line} is a citation and was not read as one"
        );
    }
    for ordinary in [
        "// the HTTP path it answers on",
        "// a run of four-R values",
        "// CRLF endings",
        "// version 2 of the record",
    ] {
        assert!(
            !cites_a_feature_requirement(ordinary),
            "{ordinary} is prose and was read as a citation"
        );
    }
}
