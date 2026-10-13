use super::{holds, FirstParty, EMBEDDED};
use crate::test_support::{an_installed, A_TREE};

fn komga(manifest: &str, tree: &str) -> crate::plugin::Installed {
    let mut installed = an_installed("komga", Vec::new());
    installed.manifest = manifest.to_owned();
    installed.tree = tree.to_owned();
    installed
}

const SET: [FirstParty; 1] = [FirstParty {
    plugin: "komga",
    manifest: "ab12",
    tree: A_TREE,
}];

#[test]
fn a_plugin_is_first_party_only_by_id_manifest_and_tree_alike() {
    assert!(holds(&SET, &komga("ab12", A_TREE)));
    assert!(!holds(&SET, &komga("cd34", A_TREE)));
    assert!(!holds(&SET, &komga("ab12", "sha256:another-tree")));
    assert!(!holds(&SET, &an_installed("kavita", Vec::new())));
    assert!(!holds(EMBEDDED, &komga("ab12", A_TREE)));
}

#[test]
fn a_pin_without_a_tree_trusts_no_install_however_recorded() {
    let set = [FirstParty { tree: "", ..SET[0] }];
    assert!(!holds(&set, &komga("ab12", "")));
}

#[test]
fn every_embedded_first_party_plugin_names_its_tree() {
    assert!(EMBEDDED.iter().all(|one| one.tree.starts_with("sha256:")));
}
