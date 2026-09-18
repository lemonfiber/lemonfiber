//! Every requirement in the tracker is claimed by exactly one row.
//!
//! Its own file because the subject is the tracker rather than the source. The
//! release gate reads done-ness from that file, so a requirement named in a ticked
//! row and an unticked one is a question the gate answers by whichever it reads
//! first — and it has been wrong that way three times, once for four releases.
//!
//! Only the identifier column is read. The prose beside it cites requirements
//! freely, which is how a row explains itself, and citing one is not claiming it.

use std::collections::BTreeMap;
use std::fs;

mod source_tree;

/// Each requirement is claimed by exactly one row of the status file.
///
/// The release gate reads done-ness from that file: a requirement named in a ✅
/// row is finished, and one named in a ☐ row is not. A requirement named in both
/// is a question the gate answers by whichever row it reads first — and it has
/// been wrong three times, once for four releases.
///
/// Only the identifier column counts. The prose beside it cites requirements
/// freely, which is how a row explains itself, and citing one is not claiming it.
#[test]
fn each_requirement_is_claimed_by_one_row() {
    // From the workspace root rather than from the working directory. The path
    // was relative and the read fell back to an empty string, so a runner that
    // started anywhere else — or a file renamed, or unreadable — left this
    // walking no rows, finding no duplicate, and passing about nothing.
    let at = source_tree::workspace_root().join("IMPLEMENTATION-STATUS.md");
    let Ok(file) = fs::read_to_string(&at) else {
        unreachable!("the workspace this test is compiled from carries {at:?}");
    };
    let mut claimed: BTreeMap<String, usize> = BTreeMap::new();
    for row in file.lines().filter(|line| line.starts_with('|')) {
        for id in requirements(column(row, 1)) {
            *claimed.entry(id).or_default() += 1;
        }
    }
    assert!(
        claimed.len() > 100,
        "the tracker was read: {} requirements claimed",
        claimed.len()
    );
    let twice: Vec<&String> = claimed
        .iter()
        .filter(|(_, rows)| **rows > 1)
        .map(|(id, _)| id)
        .collect();
    assert!(
        twice.is_empty(),
        "claimed by more than one row, so the gate reads whichever it finds first: {twice:?}"
    );
}

/// A range written backwards claims nothing, which is a row that says nothing.
///
/// `X-R4..R1` is an empty range, so the row's whole claim disappears and every
/// identifier in it reads as claimed by no row at all — which is the state this
/// file exists to make impossible, arriving by a typo rather than by a second
/// row.
#[test]
fn a_range_written_backwards_is_refused_rather_than_read_as_nothing() {
    assert_eq!(requirements("`A1-R1..R3`").len(), 3);
    assert!(
        requirements("`A1-R3..R1`").is_empty(),
        "a backwards range is what this asserts cannot appear in the tracker"
    );

    let at = source_tree::workspace_root().join("IMPLEMENTATION-STATUS.md");
    let Ok(file) = fs::read_to_string(&at) else {
        unreachable!("the workspace this test is compiled from carries {at:?}");
    };
    let backwards: Vec<&str> = file
        .lines()
        .filter(|line| line.starts_with('|'))
        .flat_map(|row| column(row, 1).split('`').skip(1).step_by(2))
        .filter(|token| reversed(token))
        .collect();
    assert!(
        backwards.is_empty(),
        "these claim nothing at all, so nothing holds them: {backwards:?}"
    );
}

/// Whether a token is a range whose end is below its start.
fn reversed(token: &str) -> bool {
    let Some((_, numbers)) = token.split_once("-R") else {
        return false;
    };
    let Some((first, last)) = numbers.split_once("..") else {
        return false;
    };
    let (Ok(first), Ok(last)) = (
        first.parse::<u32>(),
        last.trim_start_matches('R').parse::<u32>(),
    ) else {
        return false;
    };
    last < first
}
/// One cell of a table row, or nothing where the row is too short.
fn column(row: &str, at: usize) -> &str {
    row.split('|').nth(at + 1).unwrap_or_default()
}
/// Every requirement a cell claims, with `X-R1..R4` counted as the four it means.
///
/// Anything that is not a requirement identifier — a command name, a feature with
/// no requirement number, an error code — is passed over rather than guessed at.
fn requirements(cell: &str) -> Vec<String> {
    let mut found = Vec::new();
    for token in cell.split('`').skip(1).step_by(2) {
        let Some((feature, numbers)) = token.split_once("-R") else {
            continue;
        };
        if !feature
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            continue;
        }
        let (first, last) = numbers.split_once("..").unwrap_or((numbers, numbers));
        let last = last.trim_start_matches('R');
        let (Ok(first), Ok(last)) = (first.parse::<u32>(), last.parse::<u32>()) else {
            continue;
        };
        found.extend((first..=last).map(|number| format!("{feature}-R{number}")));
    }
    found
}
