//! The coverage gate and the copy of it that runs locally measure the same tree.
//!
//! The 100% gate skips a handful of files, and the list of what it skips is
//! written **twice**: once as `skipped` in the justfile, once as `SKIPPED` in
//! `sonar.yml`. Two answers to what is being measured, and nothing asked whether
//! they agree.
//!
//! One direction of a disagreement is loud — a local run that skips more than CI
//! passes here and fails there, which is annoying and safe. The other is silent:
//! widen the list CI uses and uncovered code ships with the gate green, because
//! the run that would have caught it was never measuring that file. Nobody would
//! do that deliberately; they would edit one copy and not know about the other,
//! which is what having two copies is for.
//!
//! What this cannot do is decide whether an exclusion is *justified* — that is a
//! judgement, and the justfile records the reason for each beside the list. It
//! can say the two lists are one list, and that the list still describes this
//! workspace rather than an earlier one.

mod source_tree;

use std::fs;

use source_tree::workspace_root;

/// The value of `skipped :=` in the justfile.
fn in_the_recipe(text: &str) -> Option<&str> {
    let line = text.lines().find(|line| line.starts_with("skipped :="))?;
    let (_, after) = line.split_once(":=")?;
    quoted(after)
}

/// The value of `SKIPPED:` in the coverage workflow.
fn in_the_workflow(text: &str) -> Option<&str> {
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("SKIPPED:"))?;
    let (_, after) = line.split_once(':')?;
    quoted(after)
}

/// What sits between the first pair of single quotes.
fn quoted(text: &str) -> Option<&str> {
    let (_, after) = text.split_once('\'')?;
    let (inside, _) = after.split_once('\'')?;
    Some(inside)
}

/// Both lists, read from the files that carry them.
fn both() -> (String, String) {
    let root = workspace_root();
    let recipe = fs::read_to_string(root.join("justfile")).unwrap_or_default();
    let workflow = fs::read_to_string(root.join(".github/workflows/sonar.yml")).unwrap_or_default();

    let Some(one) = in_the_recipe(&recipe) else {
        unreachable!("the justfile declares no `skipped :=`, so the gate's own list is unreadable")
    };
    let Some(other) = in_the_workflow(&workflow) else {
        unreachable!("sonar.yml declares no `SKIPPED:`, so what CI measures is unreadable")
    };
    (one.to_owned(), other.to_owned())
}

/// Both lists were read, before anything is said about them.
///
/// Two empty strings are equal, so a parser that quietly found nothing would make
/// the comparison below pass and mean nothing — which is the defect this file
/// exists to catch, one level down.
#[test]
fn the_two_lists_were_actually_read() {
    let (recipe, workflow) = both();

    assert!(
        recipe.contains("crates/"),
        "the justfile's list does not name a path in this workspace: {recipe:?}"
    );
    assert!(
        workflow.contains("crates/"),
        "the workflow's list does not name a path in this workspace: {workflow:?}"
    );
}

/// The claim: one list, written twice.
#[test]
fn the_gate_and_the_recipe_skip_the_same_files() {
    let (recipe, workflow) = both();

    assert_eq!(
        recipe, workflow,
        "the justfile and sonar.yml disagree about what the coverage gate measures, so a \
         local run and CI are answering different questions — and the direction where CI \
         skips more is silent, because the gate passes on code it never read"
    );
}

/// Every crate the list names is still here.
///
/// An exclusion for a crate that has been renamed skips nothing, which is the
/// harmless direction — but it also reads as a live decision about this workspace
/// when it is a leftover from an older one, and the next person to widen the list
/// starts from a list that is already wrong.
#[test]
fn the_list_names_only_crates_this_workspace_has() {
    let (recipe, _) = both();
    let root = workspace_root();

    let named: Vec<&str> = recipe
        .split(|character: char| !matches!(character, 'a'..='z' | '0'..='9' | '-' | '/' | '.'))
        .filter(|piece| piece.starts_with("crates/"))
        .filter_map(|piece| piece.split('/').nth(1))
        .filter(|crate_name| !crate_name.is_empty() && *crate_name != ".*")
        .collect();

    assert!(
        !named.is_empty(),
        "no crate was read out of the list, so this test is checking nothing: {recipe:?}"
    );

    let missing: Vec<&&str> = named
        .iter()
        .filter(|crate_name| !root.join("crates").join(crate_name).is_dir())
        .collect();

    assert!(
        missing.is_empty(),
        "the coverage gate excludes paths in crates this workspace does not have, so those \
         exclusions describe an older tree: {missing:?}"
    );
}
