//! A value on the wire is spelled one way.
//!
//! Field names are `snake_case` and values are `kebab-case`, so a reader of any
//! answer can tell a key from a value by its shape and never has to learn which
//! enum chose which. A new enum that serialises its variants is held to that here.
//!
//! [`FROZEN`] is what is published already under another spelling. Each entry is a
//! value somebody outside this repository reads today — a plugin author's manifest,
//! or an SDK decoding the web contract — so respelling it is a change to that
//! contract, carried by the specification and not by a refactor. An entry leaves the
//! list by the same edit that respells it.

use syn::visit::Visit;

use crate::source_tree::parsed;

/// The one spelling an enum's variants take on the wire.
const SPELLING: &str = "kebab-case";

/// Enums whose variants are published under `snake_case`, by file and name.
const FROZEN: &[(&str, &str)] = &[
    // What a plugin manifest's expectations are written in, in every plugin there is.
    ("crates/lemonfiber-plugin/src/vocabulary.rs", "Constraint"),
    // Values the web contract publishes, which the SDKs decode by name.
    ("crates/lemonfiber-core/src/backup.rs", "Scope"),
    ("crates/lemonfiber-core/src/platform.rs", "Environment"),
    ("crates/lemonfiber-core/src/platform.rs", "HostOs"),
    ("crates/lemonfiber-core/src/repair.rs", "Outcome"),
    ("crates/lemonfiber-core/src/repair.rs", "Stance"),
    ("crates/lemonfiber-core/src/repair.rs", "Writing"),
    ("crates/lemonfiber-core/src/space/category.rs", "Reclaim"),
    ("crates/lemonfiber-core/src/space/volume.rs", "Freshness"),
    ("crates/lemonfiber-core/src/space/waste.rs", "Standing"),
];

/// Every enum in shipped source that names how its variants are spelled.
fn spelled() -> Vec<(String, String, String)> {
    struct Enums {
        path: String,
        found: Vec<(String, String, String)>,
    }
    impl<'ast> Visit<'ast> for Enums {
        fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
            for attribute in &item.attrs {
                if !attribute.path().is_ident("serde") {
                    continue;
                }
                if let syn::Meta::List(list) = &attribute.meta {
                    if let Some(spelling) = rename_all(&list.tokens.to_string()) {
                        self.found
                            .push((self.path.clone(), item.ident.to_string(), spelling));
                    }
                }
            }
            syn::visit::visit_item_enum(self, item);
        }
    }
    let mut found = Vec::new();
    for (path, file) in parsed("crates") {
        let path = path.to_string_lossy().replace('\\', "/");
        if !path.contains("/src/") {
            continue;
        }
        let mut enums = Enums {
            path,
            found: Vec::new(),
        };
        enums.visit_file(&file);
        found.append(&mut enums.found);
    }
    found
}

/// The spelling a `serde` attribute's `rename_all` names, where it names one.
fn rename_all(tokens: &str) -> Option<String> {
    let (_, after) = tokens.split_once("rename_all")?;
    let (_, quoted) = after.split_once('"')?;
    let (spelling, _) = quoted.split_once('"')?;
    Some(spelling.to_owned())
}

#[test]
fn every_value_on_the_wire_is_spelled_one_way() {
    let found = spelled();
    assert!(
        found.len() > 50,
        "found {} enums naming a spelling, so the walk is looking in the wrong place",
        found.len()
    );
    let strays: Vec<String> = found
        .iter()
        .filter(|(path, name, spelling)| {
            spelling != SPELLING
                && !FROZEN
                    .iter()
                    .any(|(file, frozen)| path.ends_with(file) && name == frozen)
        })
        .map(|(path, name, spelling)| format!("{path}: {name} is {spelling}"))
        .collect();
    assert!(
        strays.is_empty(),
        "an enum spells its values other than {SPELLING}: {strays:?}"
    );
}

#[test]
fn nothing_is_frozen_that_is_already_spelled_the_one_way() {
    let found = spelled();
    let thawed: Vec<&str> = FROZEN
        .iter()
        .filter(|(file, frozen)| {
            !found.iter().any(|(path, name, spelling)| {
                path.ends_with(file) && name == frozen && spelling != SPELLING
            })
        })
        .map(|(_, frozen)| *frozen)
        .collect();
    assert!(
        thawed.is_empty(),
        "these are listed as published under another spelling, and are not: {thawed:?}"
    );
}

#[test]
fn a_spelling_is_read_off_the_attribute_that_names_it() {
    assert_eq!(
        rename_all(r#"tag = "as" , rename_all = "snake_case""#).as_deref(),
        Some("snake_case")
    );
    assert_eq!(rename_all(r#"tag = "as""#), None);
}
