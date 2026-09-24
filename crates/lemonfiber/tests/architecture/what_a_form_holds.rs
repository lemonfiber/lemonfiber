//! A fixture naming a form's services is held to the form the stack declares.
//!
//! The list exists five times over, once per test file that drives a form through
//! starting or waiting, and each copy is a second statement of something the pinned
//! stack already says. That is fine until the stack moves, and then it is the worst
//! shape a fixture can take: the fake answers only for the names it was given, the
//! code waits for the form's actual membership, and the two never meet. Nothing goes
//! red. The test hangs.
//!
//! It cost fifty-one minutes of a coverage runner when a twentieth bundled service
//! arrived on the `media` profile and five fixtures went on naming four. The failure
//! this replaces it with takes milliseconds and names the file.
//!
//! Text rather than structure, because the copies live in two crates and three of them
//! are `const`s inside a `mod tests` no other crate can reach — and what is being
//! checked is what somebody wrote down, which is exactly what a scan of the written
//! thing can see.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::source_tree::{sources, workspace_root};

/// The form every one of these fixtures is about.
///
/// One form rather than all of them, because one form is what is written down by hand:
/// `library` is the set a starting or waiting test drives, and no other form has a
/// fixture naming its members. A second one appearing here is a second one to add.
const FORM: &str = "library";

/// How a fixture names it.
const DECLARED: &str = "const LIBRARY: [&str;";

/// Every service the stack's `library` form holds, read from the pinned manifest.
///
/// Through the manifest reader rather than by matching text, so a stack that renames a
/// profile or moves a service between them is read the way lemonfiber reads it.
fn declared() -> BTreeSet<String> {
    let path = workspace_root().join("assets/media-stack/stack.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        !text.is_empty(),
        "the pinned stack is not at {} — the submodule is not checked out, so this \
         guard would be comparing every fixture against nothing",
        path.display()
    );
    // Read through the iterators rather than out of them, because an arm for a pinned
    // stack that does not parse, or that declares no such form, is a line no run can
    // enter — and the count below is what notices either.
    lemonfiber_manifest::Manifest::from_toml(&text)
        .into_iter()
        .flat_map(|manifest| {
            let profiles: BTreeSet<String> = manifest
                .forms
                .iter()
                .filter(|form| form.id == FORM)
                .flat_map(|form| form.profiles.iter().cloned())
                .collect();
            manifest
                .services
                .into_iter()
                .filter(|service| profiles.contains(&service.profile))
                .map(|service| service.id)
                .collect::<Vec<String>>()
        })
        .collect()
}

/// Every fixture in the workspace that names the form's services, and what it names.
///
/// The array's contents, read between the brackets that follow the declaration. A
/// fixture written some other way is not found, which is the honest limit of a scan —
/// and the reason this counts what it found before concluding anything from it.
fn fixtures() -> Vec<(PathBuf, BTreeSet<String>)> {
    let mut found = Vec::new();
    for (path, text) in sources() {
        // This file is left out, for the reason every source-reading guard leaves
        // itself out: it writes the declaration down in order to look for it, so a
        // sweep that read itself would report its own scanner as a broken fixture.
        if path.ends_with("what_a_form_holds.rs") {
            continue;
        }
        for after in text.split(DECLARED).skip(1) {
            let Some((_, inside)) = after.split_once('[') else {
                continue;
            };
            let Some((listed, _)) = inside.split_once(']') else {
                continue;
            };
            let named: BTreeSet<String> = listed
                .split(',')
                .map(|piece| piece.trim().trim_matches('"').to_owned())
                .filter(|piece| !piece.is_empty())
                .collect();
            found.push((path.clone(), named));
        }
    }
    found
}

/// The scan found the fixtures it is about, before anything is concluded from them.
///
/// An empty list agrees with everything, so a scan that quietly found nothing would
/// make the comparison below pass and mean nothing — which is one level down the same
/// failure this file exists to catch.
#[test]
fn the_fixtures_this_is_about_were_actually_found() {
    let found = fixtures();
    assert!(
        found.len() >= 5,
        "this found {} fixtures naming `{FORM}` and there are at least five — the \
         declaration has been written some other way and the ones it no longer sees \
         are unguarded: {:?}",
        found.len(),
        found.iter().map(|(path, _)| path).collect::<Vec<_>>()
    );
    assert!(
        declared().len() > 1,
        "the `{FORM}` form was read as holding fewer than two services, which is not a \
         form"
    );
}

/// Each fixture names exactly what the form holds.
///
/// Both directions in one comparison, because both hang: a fixture short of a service
/// leaves the code waiting for one that never answers, and a fixture carrying one the
/// form does not hold has a fake standing in for something nothing asks about.
#[test]
fn every_fixture_names_exactly_what_the_form_declares() {
    let holds = declared();
    let wrong: Vec<String> = fixtures()
        .into_iter()
        .filter(|(_, named)| *named != holds)
        .map(|(path, named)| {
            let missing: Vec<&String> = holds.difference(&named).collect();
            let extra: Vec<&String> = named.difference(&holds).collect();
            format!("{}: missing {missing:?}, extra {extra:?}", path.display())
        })
        .collect();

    assert!(
        wrong.is_empty(),
        "these name a different set of services than the `{FORM}` form the pinned stack \
         declares, so a test driving that form waits on a service nothing answers for: \
         {wrong:?}"
    );
}
