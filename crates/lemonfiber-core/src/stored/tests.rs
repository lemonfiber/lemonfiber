use std::path::Path;

use super::{stored, Removal, EVERY};
use crate::config::paths::Paths;

fn a_layout() -> Paths {
    Paths::rooted(
        Path::new("/home/op/.config"),
        Path::new("/home/op/.local/share"),
    )
}

#[test]
fn everything_kept_is_named_where_it_is_and_why() {
    let disclosed = stored(&a_layout(), Removal::NotAsked);

    assert_eq!(disclosed.kept.len(), EVERY.len());
    for entry in &disclosed.kept {
        assert!(entry.at.starts_with("/home/op/"), "{entry:?}");
        assert!(
            entry.why.split_whitespace().count() >= 8,
            "{} says nothing an operator could act on",
            entry.what
        );
    }
}

/// The claim removal rests on: two directories, and everything under one of
/// them. Asserted here as well as in the layout's own tests, because this is
/// where it is being *relied on* — forgetting removes two trees and calls that
/// all of it.
#[test]
fn everything_kept_is_under_one_of_the_two_directories() {
    let paths = a_layout();
    let disclosed = stored(&paths, Removal::NotAsked);
    let roots: Vec<String> = disclosed.roots.iter().map(|root| root.at.clone()).collect();

    assert_eq!(roots.len(), 2, "{roots:?}");
    let outside: Vec<&str> = disclosed
        .kept
        .iter()
        .map(|entry| entry.at.as_str())
        .filter(|at| !roots.iter().any(|root| at.starts_with(root)))
        .collect();
    assert!(
        outside.is_empty(),
        "these are kept somewhere removing the two roots would not reach: {outside:?}"
    );
}

#[test]
fn what_is_not_lemonfibers_is_named_with_whose_it_is() {
    let disclosed = stored(&a_layout(), Removal::NotAsked);

    assert!(disclosed.beside.len() >= 3);
    let said = disclosed
        .beside
        .iter()
        .map(|beside| format!("{} {}", beside.what, beside.why))
        .collect::<Vec<String>>()
        .join(" ");
    assert!(said.contains("library"), "{said}");
}

#[test]
fn a_listing_says_nobody_asked_for_anything_to_be_removed() {
    assert_eq!(
        stored(&a_layout(), Removal::NotAsked).removal,
        Removal::NotAsked
    );
}

#[test]
fn the_credentials_are_marked_as_such_and_not_everything_is() {
    let disclosed = stored(&a_layout(), Removal::NotAsked);
    let holding = disclosed.kept.iter().filter(|entry| entry.secret).count();

    assert!(
        holding > 0,
        "nothing here is marked as holding a credential"
    );
    assert!(holding < disclosed.kept.len(), "everything is marked");
}
