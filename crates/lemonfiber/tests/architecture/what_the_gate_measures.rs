//! The coverage gate and the copy of it that runs locally measure the same tree.
//!
//! The 100% gate skips a handful of files, and what it skips is written once, in
//! `.config/coverage-skipped`. The justfile's `skipped` and the workflow's `SKIPPED`
//! both read that file, so a local run and CI answer the same question — and the
//! silent direction of a disagreement, CI skipping a file a local run measures,
//! cannot be written.
//!
//! What this cannot do is decide whether an exclusion is *justified* — that is a
//! judgement, and the justfile records the reason for each beside the recipe. It can
//! say both readers read the one list, and that the list still describes this
//! workspace.
//!
//! What a *failing* gate is asked is written in two places — as `just uncovered` and
//! as the step `sonar.yml` runs on failure — and those are held to each other below.

use std::fs;

use crate::source_tree::workspace_root;

/// Where the list of what the gate skips is written.
const LIST: &str = ".config/coverage-skipped";

/// The justfile and the coverage workflow, as they are written.
fn files() -> (String, String) {
    let root = workspace_root();
    (
        fs::read_to_string(root.join("justfile")).unwrap_or_default(),
        fs::read_to_string(root.join(".github/workflows/sonar.yml")).unwrap_or_default(),
    )
}

/// The list itself, as the file holds it.
fn list() -> String {
    fs::read_to_string(workspace_root().join(LIST))
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// Both the recipe and the workflow read the one list.
#[test]
fn the_gate_and_the_recipe_read_the_one_list() {
    let (recipe, workflow) = files();

    assert!(
        recipe
            .lines()
            .any(|line| line.starts_with("skipped :=") && line.contains(LIST)),
        "the justfile's `skipped` is not read from {LIST}, so a local run can measure a \
         different tree from CI"
    );
    assert!(
        workflow
            .lines()
            .any(|line| line.contains("SKIPPED=") && line.contains(LIST)),
        "sonar.yml's `SKIPPED` is not read from {LIST}, so CI can skip what a local run \
         measures — the direction where the gate passes on code it never read"
    );
}

/// The list was read, before anything is said about it.
///
/// An empty list skips nothing and names nothing, so a list that quietly went missing
/// would pass every check below while meaning nothing.
#[test]
fn the_list_was_actually_read() {
    let skipped = list();

    assert!(
        skipped.contains("crates/"),
        "{LIST} does not name a path in this workspace: {skipped:?}"
    );
    assert!(
        !skipped.contains('\n'),
        "{LIST} holds more than one line, and both readers take it as one pattern"
    );
}

/// Every crate the list names is still here.
///
/// An exclusion for a crate that has been renamed skips nothing, which is the
/// harmless direction — but it also reads as a live decision about this workspace
/// when it describes another one, and the next person to widen the list starts from
/// a list that is already wrong.
#[test]
fn the_list_names_only_crates_this_workspace_has() {
    let skipped = list();
    let root = workspace_root();

    let named: Vec<&str> = skipped
        .split(|character: char| !matches!(character, 'a'..='z' | '0'..='9' | '-' | '/' | '.'))
        .filter(|piece| piece.starts_with("crates/"))
        .filter_map(|piece| piece.split('/').nth(1))
        .filter(|crate_name| !crate_name.is_empty() && *crate_name != ".*")
        .collect();

    assert!(
        !named.is_empty(),
        "no crate was read out of the list, so this test is checking nothing: {skipped:?}"
    );

    let missing: Vec<&&str> = named
        .iter()
        .filter(|crate_name| !root.join("crates").join(crate_name).is_dir())
        .collect();

    assert!(
        missing.is_empty(),
        "the coverage gate excludes paths in crates this workspace does not have, so those \
         exclusions describe another tree: {missing:?}"
    );
}

/// The two questions a failing gate is asked, each named by what asks it.
///
/// One reads the export's merged segments and one reads its regions. They are a pair
/// rather than a first choice and a fallback: the segments are what a reader wants
/// when they speak, and they are silent on exactly the miss that survives a merge.
const ASKED: [&str; 2] = ["--show-missing-lines", "counted_but_not_named.py"];

/// A failing gate is asked the same questions from a shell and from CI.
///
/// The direction that matters is CI asking less: the segments half comes back empty
/// on exactly the failure the regions half exists for, and CI is where the gate fails
/// while a shell is where somebody has to reproduce it.
#[test]
fn a_failing_gate_is_asked_the_same_questions_in_both_places() {
    let (recipe, workflow) = files();

    for asks in ASKED {
        assert!(
            recipe.contains(asks),
            "`just uncovered` no longer asks {asks}, so a gate that fails from a shell \
             answers with less than the one that fails in CI"
        );
        assert!(
            workflow.contains(asks),
            "the coverage workflow no longer asks {asks}, so a gate that fails in CI \
             answers with less than the one that fails from a shell"
        );
    }
}

/// The reader both of them name is a file that is here.
///
/// Both reach it through a pipe, and a pipe whose right-hand side does not exist says
/// so to a stream nobody is reading: the recipe swallows the failure with `-`, and
/// the workflow step is already running because something else went red. A renamed
/// script would leave both halves reporting nothing and neither saying why.
#[test]
fn the_reader_they_name_is_here() {
    let reader = workspace_root().join("scripts/counted_but_not_named.py");

    assert!(
        reader.is_file(),
        "the recipe and the workflow both pipe a report into {} and it is not there, so \
         the half that can name a miss the segments merge away answers nothing",
        reader.display()
    );
}
