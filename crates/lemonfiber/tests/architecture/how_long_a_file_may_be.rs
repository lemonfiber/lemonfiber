//! How long a file may be, counted over every line in it.
//!
//! Two caps, because a test file and a shipped file are held to different lengths
//! for different reasons, and each is measured over the whole file. A source file's
//! tests are a file of their own beside it, so a source file is only what ships and
//! nothing a cap could stop counting at.

use std::path::{Path, PathBuf};

use crate::source_tree::sources;

/// Lines a shipped source file may hold.
const SOURCE_CAP: usize = 550;

/// Lines a test file may hold.
///
/// Longer than the source cap because a test file carries fixtures and every case
/// of one thing; past this it has stopped being the tests for one thing and become
/// the tests for a subsystem, which is a file to split by what it asserts.
const TEST_CAP: usize = 800;

/// Whether a file is a test: a `tests.rs` beside the module it tests, or anything
/// under a `tests` directory.
fn is_test(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "tests.rs")
        || path.components().any(|part| part.as_os_str() == "tests")
}

/// Every file over the cap it is held to, longest first.
fn oversized(testing: bool, cap: usize) -> Vec<(PathBuf, usize)> {
    let mut over: Vec<(PathBuf, usize)> = sources()
        .into_iter()
        .filter(|(path, _)| is_test(path) == testing)
        .map(|(path, text)| (path, text.lines().count()))
        .filter(|(_, lines)| *lines > cap)
        .collect();
    over.sort_by_key(|(_, lines)| std::cmp::Reverse(*lines));
    over
}

#[test]
fn no_source_file_outgrows_reading_in_one_sitting() {
    let over = oversized(false, SOURCE_CAP);
    assert!(
        over.is_empty(),
        "past {SOURCE_CAP} lines, split into modules by concept: {over:?}"
    );
}

#[test]
fn no_test_file_covers_more_than_one_seam() {
    let over = oversized(true, TEST_CAP);
    assert!(
        over.is_empty(),
        "past {TEST_CAP} lines a test file covers more than one seam — split it by what \
         it asserts: {over:?}"
    );
}

/// A source file keeps its tests in a file beside it rather than inside it, so the
/// source cap measures what ships and the test cap measures the tests.
#[test]
fn a_source_file_keeps_its_tests_beside_it() {
    let inline: Vec<PathBuf> = sources()
        .into_iter()
        .filter(|(path, _)| !is_test(path))
        .filter(|(_, text)| {
            text.lines()
                .zip(text.lines().skip(1))
                .any(|(gate, module)| {
                    let module = module.trim_start();
                    gate.trim() == "#[cfg(test)]"
                        && (module.starts_with("mod ") || module.starts_with("pub(crate) mod "))
                        && module.ends_with('{')
                })
        })
        .map(|(path, _)| path)
        .collect();
    assert!(
        inline.is_empty(),
        "these hold a test module inline; move it to `tests.rs` beside them: {inline:?}"
    );
}
