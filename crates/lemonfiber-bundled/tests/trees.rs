//! Every pinned copy, read as a plugin's repository is, has the tree its pin names.

use std::path::Path;

const CATALOGUE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/plugins");

#[test]
fn every_copy_read_as_a_repository_has_the_tree_its_pin_names() {
    let root = Path::new(CATALOGUE);
    let Ok((_, held)) = lemonfiber_bundle::read(root) else {
        unreachable!("the embedded bundle reads");
    };
    assert!(!held.is_empty());
    for one in &held {
        let copy = root.join(lemonfiber_bundle::COPIES).join(&one.pin.id);
        assert_eq!(
            lemonfiber_bundle::tree_of(&copy, &one.copy).ok(),
            one.pin.tree,
            "{}",
            one.pin.id
        );
    }
}

#[test]
fn an_installed_plugin_carries_the_tree_its_pin_names_and_an_assembled_one_none() {
    let mut installed = 0;
    for pinned in lemonfiber_bundled::PINNED {
        let files: std::collections::BTreeMap<String, Vec<u8>> = pinned
            .files
            .iter()
            .map(|(path, bytes)| ((*path).to_owned(), bytes.to_vec()))
            .collect();
        match pinned.stage {
            Some(lemonfiber_bundled::Stage::Installed | lemonfiber_bundled::Stage::Offered) => {
                installed += 1;
                assert_eq!(
                    lemonfiber_bundle::tree_digest(pinned.copy, &files),
                    pinned.first_party.tree,
                    "{}",
                    pinned.first_party.plugin
                );
            }
            _ => assert!(files.is_empty(), "{}", pinned.first_party.plugin),
        }
        assert!(pinned.profile.is_some(), "{}", pinned.first_party.plugin);
    }
    assert!(installed > 0);
}

#[test]
fn every_part_the_bundle_pins_is_installed_through_the_plugin_path() {
    let assembled: Vec<&str> = lemonfiber_bundled::PINNED
        .iter()
        .filter(|pinned| {
            !matches!(
                pinned.stage,
                Some(lemonfiber_bundled::Stage::Installed | lemonfiber_bundled::Stage::Offered)
            )
        })
        .map(|pinned| pinned.first_party.plugin)
        .collect();
    assert!(!lemonfiber_bundled::PINNED.is_empty());
    assert!(assembled.is_empty(), "{assembled:?}");
}
