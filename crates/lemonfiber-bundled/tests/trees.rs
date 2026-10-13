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
