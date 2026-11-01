use super::{outermost_missing, pinned, taken_away, written};

#[test]
fn only_a_plugin_the_bundle_installs_is_read_from_this_build() {
    assert!(pinned("jellyfin").is_some());
    assert!(pinned("flaresolverr").is_none());
    assert!(pinned("no-such-plugin").is_none());
}

#[test]
fn the_tree_written_out_is_the_tree_the_pin_names() {
    let Some(jellyfin) = pinned("jellyfin") else {
        unreachable!("the bundle installs jellyfin");
    };
    let scratch = lemonfiber_fixtures::scratch::Scratch::named("embedded-tree");
    let into = scratch.path().join("checkout");
    assert!(written(&into, jellyfin).is_ok());
    assert_eq!(
        lemonfiber_bundle::tree_of(&into, jellyfin.copy)
            .ok()
            .as_deref(),
        Some(jellyfin.first_party.tree)
    );
}

/// A checkout goes with every directory it made on the way to it, and leaves the ones
/// that were there before or that hold something written since.
#[test]
fn a_checkout_goes_with_the_directories_it_made_and_no_other() {
    let root = lemonfiber_fixtures::scratch::Scratch::new("embedding-taken-away");
    let into = root.join("data/checkouts/one");
    let outermost = outermost_missing(&into);
    let _ = std::fs::create_dir_all(into.join("tree"));
    taken_away(&into, &outermost);
    assert_eq!(outermost, root.join("data"));
    assert!(!root.join("data").exists());
    assert!(root.path().is_dir());

    let beside = root.join("data/checkouts/other");
    let outermost = outermost_missing(&into);
    let _ = std::fs::create_dir_all(&into);
    let _ = std::fs::create_dir_all(&beside);
    taken_away(&into, &outermost);
    assert!(!into.exists());
    assert!(beside.is_dir());
}
