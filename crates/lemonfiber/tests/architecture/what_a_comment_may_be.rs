//! What a comment in this workspace may be, read the way the compiler reads it.
//!
//! **The rules read tokens, not lines.** `let s = "// not a comment";` is code, and
//! `let x = 1; // narration` is a comment — a scan that decides by whether a trimmed
//! line opens with two slashes gets the first wrong and cannot see the second at
//! all. A rule that silently ignores a whole comment position is worse than no rule,
//! because the tree reads as checked, and this workspace had exactly one comment
//! standing in the position nothing could see.
//!
//! **Every rule runs against a tree of planted comments**, where each must find its
//! own and no other. A rule that has only ever seen clean code has never been shown
//! to work, and goes on proving nothing right up to the first file that needed it.
//!
//! **Five of the seven also run against the tree, where they must find nothing.** No
//! note is written after code, no comment is a block comment, none defers work, none
//! cites a requirement, and every page under `.docs/` that one names is a page that
//! exists. The other two are the length rules, and what this workspace does with
//! them is written where they are asked.

use crate::lexed;
use std::path::PathBuf;

use crate::lexed::{comments, Rule, Violation, RULES};
use crate::source_tree::{shipped, sources, workspace_root};

/// Which rule each planted file is for, and the two that are for none.
///
/// The two are what makes the table a matrix rather than a list: a rule has to pass
/// the correct file as well as fail the wrong one, and `nothing_a_string_holds` is
/// the one that says whether the reader is lexing at all.
const PLANTED: &[(&str, &str)] = &[
    ("a_block_comment", "a block comment"),
    ("a_deferral_token", "a deferral token"),
    ("a_note_after_code", "a note written after code"),
    ("a_note_standing_alone", "a note standing alone"),
    ("a_page_that_is_not_there", "a page that is not there"),
    ("a_requirement_identifier", "a requirement identifier"),
    (
        "a_run_that_is_too_long",
        "a run longer than prose belongs in",
    ),
    ("compliant", ""),
    ("nothing_a_string_holds", ""),
];

/// Where the planted files are kept.
fn planted() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/comment_policy")
}

/// One planted file, read as text.
fn fixture(named: &str) -> String {
    std::fs::read_to_string(planted().join(format!("{named}.rs.fixture"))).unwrap_or_default()
}

/// What one rule found in one file, said the way a failure would read.
fn named(found: &[Violation], at: &str) -> String {
    found
        .iter()
        .map(|one| format!("  {at}:{} — {}", one.line, one.said))
        .collect::<Vec<String>>()
        .join("\n")
}

/// Every rule finds the comment planted for it, and finds nothing in the rest.
///
/// A rule that fires on the compliant file has learned to refuse correct code, and a
/// rule that fires on somebody else's planted file is two rules wearing one name —
/// neither shows up in a sweep over a tree that is already clean.
#[test]
fn each_rule_finds_its_own_planted_comment_and_no_other() {
    let root = workspace_root();
    let mut asked = 0_usize;
    for (file, owner) in PLANTED {
        let text = fixture(file);
        assert!(!text.is_empty(), "{file} is not a planted file");
        for (rule, holds) in RULES {
            let found = holds(&text, &root);
            let said = named(&found, file);
            if rule == owner {
                assert!(
                    !found.is_empty(),
                    "{rule} did not find what {file} plants for it"
                );
            } else {
                assert!(found.is_empty(), "{rule} fired on {file}:\n{said}");
            }
            asked += 1;
        }
    }
    let expected = PLANTED.len() * RULES.len();
    assert_eq!(asked, expected, "the matrix was not filled in");
}

/// The planted files and the table naming them are the same set.
///
/// A file nobody owns is a violation nothing looks for, and a row with no file is a
/// rule the matrix silently stops exercising. Neither shows up as a failure in the
/// matrix itself, because the matrix only ever reads what the table names.
#[test]
fn every_planted_file_is_named_and_every_name_is_a_file() {
    let mut found: Vec<String> = std::fs::read_dir(planted())
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter_map(|name| name.strip_suffix(".rs.fixture").map(str::to_owned))
        .collect();
    found.sort();

    let mut table: Vec<String> = PLANTED.iter().map(|(file, _)| (*file).to_owned()).collect();
    table.sort();

    assert!(
        found.len() > 5,
        "the planted tree holds {} files",
        found.len()
    );
    assert_eq!(found, table, "the planted files and the table disagree");
}

/// Nothing a literal holds is read as a comment.
///
/// The property the whole reader exists for, asserted on what it found rather than
/// on what the rules made of it: the planted file carries two slashes, a deferral
/// token, two requirement identifiers and a missing page, every one of them inside a
/// string, a raw string with hashes, a byte string or a character literal.
#[test]
fn nothing_a_literal_holds_is_read_as_a_comment() {
    let found = comments(&fixture("nothing_a_string_holds"));
    let doc = found.iter().filter(|one| one.doc).count();

    assert_eq!(found.len(), doc, "a literal was read as a comment");
    assert_eq!(doc, 4, "the reader lost a doc comment, or invented one");
    assert!(
        found
            .iter()
            .any(|one| one.block && one.said.contains("/* block */")),
        "a nested block comment did not close where its own marker is"
    );
}

/// No comment in the shipped tree is a block comment.
#[test]
fn nothing_shipped_writes_a_block_comment() {
    swept(lexed::block_comment, &shipped());
}

/// No comment anywhere says the work is unfinished.
///
/// The whole tree rather than the shipped half. A deferral in a test is the same
/// unfinished work under a name that happens not to ship.
#[test]
fn nothing_defers_the_work_to_a_comment() {
    swept(lexed::deferral_token, &everything());
}

/// No comment anywhere cites a requirement.
///
/// Provenance in a comment is worthless to the next reader and rots the moment the
/// requirement is superseded. Identifiers go in commits and pull requests; code
/// links to `.docs/`, and those pages cite the specification.
#[test]
fn nothing_cites_a_requirement_in_a_comment() {
    swept(lexed::requirement_identifier, &everything());
}

/// Every page a comment names is a page that is there.
///
/// Named in whichever of the three forms this workspace writes one: a backticked
/// path, a Markdown link, and the address of the same file on the forge. A renamed
/// page would otherwise leave every mention of it pointing at nothing, silently.
#[test]
fn every_page_a_comment_names_is_one_that_exists() {
    swept(lexed::unresolved_doc_link, &everything());
}

/// No note is written after code.
///
/// The position nothing could see, and the one where a one-line note is reliably
/// narration rather than a reason: a value restated, a call named twice. There was
/// one in this tree and it said what the constant beside it already meant.
#[test]
fn nothing_shipped_writes_a_note_after_code() {
    swept(lexed::note_after_code, &shipped());
}

/// The two rules this tree does not meet still read it.
///
/// A note standing alone and a run past four lines are the house style here, not a
/// backlog. The lone ones are match arms carrying the one clause that says why it is
/// that arm; the long ones are ordering traps and defended-against edge cases that
/// take three paragraphs to state, and they are among the best comments in this
/// repository. Holding the tree to either would mean rewriting two hundred of them
/// into padding or into pages — a change of house style, which is not a gate's to
/// make.
///
/// So neither is swept, and this is what stands in its place: both rules are proved
/// against their own planted files above, and both are asked of the real tree here
/// so that a reader which quietly stopped finding anything is caught. A floor rather
/// than a ceiling — it goes red when the rule breaks, never when somebody writes a
/// comment, which is the failure a count of today's violations would have become.
#[test]
fn the_two_rules_this_tree_does_not_meet_still_read_it() {
    let corpus = shipped();
    let alone = across(lexed::lone_comment, &corpus).len();
    let long = across(lexed::overlong_block, &corpus).len();

    assert!(
        alone > 20,
        "the standing-alone rule found {alone} in this tree, which means it has \
         stopped reading it"
    );
    assert!(
        long > 20,
        "the run-length rule found {long} in this tree, which means it has stopped \
         reading it"
    );
}

/// Every `.rs` file in the workspace, keyed by the path a failure names.
fn everything() -> std::collections::BTreeMap<String, String> {
    sources()
        .into_iter()
        .map(|(path, text)| (path.to_string_lossy().replace('\\', "/"), text))
        .collect()
}

/// One rule over one corpus, which must find nothing.
fn swept(holds: Rule, corpus: &std::collections::BTreeMap<String, String>) {
    let found = across(holds, corpus);
    let said = found.join("\n");
    assert!(found.is_empty(), "{said}");
}

/// What one rule found across a corpus, each said with where it is.
fn across(holds: Rule, corpus: &std::collections::BTreeMap<String, String>) -> Vec<String> {
    let root = workspace_root();
    assert!(
        corpus.len() > 10,
        "the corpus holds {} files, which means it is reading the wrong tree",
        corpus.len()
    );
    corpus
        .iter()
        .flat_map(|(path, text)| {
            holds(text, &root)
                .into_iter()
                .map(move |one| format!("{path}:{} — {}", one.line, one.said))
        })
        .collect()
}

/// The identifier rule reads a feature number of any length, and prose as prose.
///
/// Every subject is assembled from its parts rather than written out, because a
/// literal one would be a citation in this file and the sweep above reads this file.
/// What is worth pinning is the two-digit feature number: a rule that catches the
/// shorter shape and not the longer one goes green over exactly the citations nobody
/// notices, which is what it did until the window became a scan.
#[test]
fn the_identifier_rule_reads_a_feature_number_of_any_length() {
    let root = workspace_root();
    let cited = |area: &str, feature: u32, index: u32| {
        format!("// a note about {area}{feature}-R{index}\n// and its second line\n")
    };

    let mut asked = 0_usize;
    for (area, feature) in [("F", 1_u32), ("F", 10), ("B", 10), ("D", 10), ("F", 11)] {
        let line = cited(area, feature, 4);
        assert!(
            !lexed::requirement_identifier(&line, &root).is_empty(),
            "{line} is a citation and was not read as one"
        );
        asked += 1;
    }
    assert_eq!(asked, 5, "no citation was put in front of the rule");

    for ordinary in [
        "// the HTTP path it answers on\n// and what it answers with\n",
        "// a run of four-R values\n// read in the order they arrive\n",
        "// CRLF endings\n// which the reader keeps\n",
        "// version 2 of the record\n// which is the one this writes\n",
    ] {
        assert!(
            lexed::requirement_identifier(ordinary, &root).is_empty(),
            "{ordinary} is prose and was read as a citation"
        );
    }
}
