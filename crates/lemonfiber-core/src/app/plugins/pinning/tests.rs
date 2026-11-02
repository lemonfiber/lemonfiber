use super::from_its_pin;
use crate::error::codes::plugin::ANOTHER_SOURCE;
use crate::plugin::first_party::FirstParty;
use crate::test_support::{an_installed, A_TREE};

const PINNED: [FirstParty; 1] = [FirstParty {
    plugin: "komga",
    manifest: "ab12",
    tree: A_TREE,
}];

fn offered(plugin: &str, manifest: &str, tree: &str) -> crate::plugin::Installed {
    let mut installed = an_installed(plugin, Vec::new());
    installed.manifest = manifest.to_owned();
    installed.tree = tree.to_owned();
    installed
}

#[test]
fn a_pinned_id_from_its_pin_is_let_through() {
    assert!(from_its_pin(&offered("komga", "ab12", A_TREE), &PINNED).is_ok());
}

#[test]
fn a_pinned_id_from_anywhere_else_is_refused_naming_it_and_the_build_carrying_it() {
    for (manifest, tree) in [
        ("cd34", A_TREE),
        ("ab12", "sha256:another-tree"),
        ("cd34", ""),
    ] {
        let refused = from_its_pin(&offered("komga", manifest, tree), &PINNED).err();
        let code = refused.as_ref().map(|problem| problem.code);
        let message = refused.map(|problem| problem.summary).unwrap_or_default();
        assert_eq!(code, Some(ANOTHER_SOURCE));
        assert!(message.contains("komga"), "{message}");
        assert!(message.contains("this build carries"), "{message}");
    }
}

#[test]
fn an_id_no_pin_names_comes_from_wherever_it_was_offered() {
    assert!(from_its_pin(&offered("kavita", "cd34", "sha256:another-tree"), &PINNED).is_ok());
}
