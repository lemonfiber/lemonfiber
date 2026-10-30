use super::{pinned, written};

#[test]
fn only_a_plugin_the_bundle_installs_is_read_from_this_build() {
    assert!(pinned("jellyfin").is_some());
    assert!(pinned("caddy").is_none());
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
