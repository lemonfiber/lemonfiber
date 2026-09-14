//! How long a file may be, and where the line that ends the counting sits.
//!
//! The caps live here rather than among the rules they used to sit with, and the
//! reason is the obvious one: the file enforcing a length cap had grown to fifty
//! lines short of breaching it. A rule whose own home is about to fail it is a rule
//! nobody can take at face value, and the answer the rule itself gives — split it —
//! is the one it had to be given.
//!
//! Three rules, one subject. Two caps, because a test file and a shipped file are
//! held to different lengths for different reasons, and the third is what keeps the
//! production cap honest: it counts up to the test module, so a file declaring its
//! tests somewhere the counter does not look would be measuring half of itself.

use std::path::PathBuf;

mod source_tree;

use source_tree::{production, sources};

/// A test file covers one seam, and stays small enough to read.
///
/// The production cap does not apply here: a test file legitimately carries
/// fixtures, fakes and every case of one thing, and holding it to 550 would push
/// the shared scaffolding into ever more modules rather than making anything
/// clearer.
///
/// What does apply is the reason behind that cap. One file, one seam. Past this
/// length a file has stopped being the tests for one thing and become the tests
/// for a subsystem — which is how `seed.rs` reached two and a half thousand lines
/// covering five different drivers, each with a fake nobody else could see.
///
/// The number is a ratchet, not a target: it should come down as files are split,
/// never up to admit one that grew.
#[test]
fn no_test_file_covers_more_than_one_seam() {
    /// Lines a single test file may hold.
    const CAP: usize = 1_200;

    let oversized: Vec<(PathBuf, usize)> = sources()
        .into_iter()
        .filter(|(path, _)| path.to_string_lossy().contains("tests"))
        .map(|(path, text)| (path, text.lines().count()))
        .filter(|(_, lines)| *lines > CAP)
        .collect();
    assert!(
        oversized.is_empty(),
        "past {CAP} lines a test file covers more than one seam — split it: {oversized:?}"
    );
}
/// No file grows past what one sitting can hold.
///
/// A file that keeps accreting is how a codebase stops being navigable: the third
/// concern arrives, nobody notices, and by the fifth there is nowhere obvious to
/// put the sixth. The cap is deliberately mechanical — it makes no judgement about
/// whether a file is *cohesive*, only that it is finite — and it is a floor under
/// review rather than a substitute for it.
///
/// Counted before the test module, because tests legitimately double a file and
/// there is no reason to ration them.
///
/// When this fails, the answer is a module: split the file into a directory of the
/// same name, one concern per file, tests beside the code they exercise and shared
/// fixtures in a `fixtures.rs` of their own. Raising the number is not the answer.
#[test]
fn no_source_file_outgrows_reading_in_one_sitting() {
    /// Production lines a single file may hold.
    const CAP: usize = 550;

    let mut oversized: Vec<(PathBuf, usize)> = Vec::new();
    for (path, text) in sources() {
        if path.to_string_lossy().contains("tests") {
            continue;
        }
        let shipped = production(&text).lines().count();
        if shipped > CAP {
            oversized.push((path, shipped));
        }
    }
    oversized.sort_by_key(|(_, lines)| std::cmp::Reverse(*lines));

    let named: Vec<String> = oversized
        .iter()
        .map(|(path, lines)| format!("{} ({lines})", path.display()))
        .collect();
    assert!(
        oversized.is_empty(),
        "past {CAP} production lines, split into a module rather than raising the cap: {}",
        named.join(", ")
    );
}
/// The name a `mod NAME {` line declares, where it opens one here.
///
/// Only an inline module counts. A `mod name;` declaration puts the code in another
/// file, which the cap measures on its own, so it is not a point this file stops
/// shipping at — `prompt.rs` and the core's `lib.rs` both declare a test-only module
/// that way and are production all the way down.
fn module_name(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("pub(crate) ").unwrap_or(trimmed);
    let opened = rest.strip_prefix("mod ")?.trim_end().strip_suffix('{')?;
    let name = opened.trim_end();
    (!name.is_empty()).then_some(name)
}
/// The modules a file declares behind `#[cfg(test)]`, in the order they appear.
fn test_modules(text: &str) -> Vec<&str> {
    text.lines()
        .zip(text.lines().skip(1))
        .filter(|(attribute, _)| attribute.trim() == "#[cfg(test)]")
        .filter_map(|(_, declaration)| module_name(declaration))
        .collect()
}
/// A file that has tests declares them somewhere the line cap can find them.
///
/// The cap tells production from test by looking for a `mod tests`, so a lone test
/// module under any other name leaves the *whole* file counted as shipped. That is
/// not theoretical: `services.rs` measured 538 of 550 while shipping 372 lines,
/// because its tests were `mod telling_tests`. The next person to add a dozen lines
/// of test to it would have been told to split a file that had 178 lines spare —
/// by a cap whose own reason for existing says tests are not rationed.
///
/// Conservative in direction, which is why it went unnoticed: over-counting only
/// ever produces a false red. It still makes the cap arbitrary from file to file,
/// and a guard nobody can predict is one people learn to raise rather than obey.
///
/// A *second* test module beside the first may be named for what it covers —
/// `exit.rs` has `mod reporting` after its `mod tests`, and the cap has already cut
/// by then. Only the declaration it cuts at has to be findable.
#[test]
fn a_file_with_tests_declares_them_where_the_line_cap_looks() {
    let mut unfindable: Vec<String> = Vec::new();
    for (path, text) in sources() {
        if path.to_string_lossy().contains("tests") {
            continue;
        }
        let declared = test_modules(&text);
        if declared.is_empty() || declared.contains(&"tests") {
            continue;
        }
        unfindable.push(format!(
            "{} (mod {})",
            path.display(),
            declared.join(", mod ")
        ));
    }
    assert!(
        unfindable.is_empty(),
        "the line cap finds where a file stops shipping by its `mod tests`, so these \
         have their tests counted as production: {}",
        unfindable.join(", ")
    );
}
