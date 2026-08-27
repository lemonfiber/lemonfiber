//! What is compiled into this binary, asserted rather than assumed.
//!
//! Both embedded trees arrive as pinned submodules and are compiled in with
//! `include_dir!`, which is happy to embed an empty directory: a checkout whose
//! submodules are not populated builds and carries nothing. That is deliberate —
//! the repository has to be workable without them — and it means the difference
//! between carrying the app and not carrying it is invisible to the compiler.
//!
//! It has already gone wrong once. `assets/web` was pinned and its wire version
//! checked at build time while the constant naming it was still `None`, so the
//! app was validated and never served, and every test passed. Nothing here read
//! what the binary carries, so nothing could say.

use lemonfiber::cli::{APP, STACK};

/// The app is carried, and it is the app rather than an empty directory.
///
/// `index.html` is what a browser asking for the root is answered with, and
/// `app.json` is what `build.rs` reads to refuse a version this binary does not
/// serve — a tree missing either is not one worth shipping.
#[test]
fn the_binary_carries_the_app_a_browser_is_served() {
    assert!(
        APP.get_file("index.html").is_some(),
        "the app has no index.html, so a browser asking for the root gets nothing"
    );
    assert!(
        APP.get_file("app.json").is_some(),
        "the app declares no wire version, so the build-time check reads nothing"
    );
}

/// The stack is carried too, held the same way for the same reason.
#[test]
fn the_binary_carries_the_stack_it_operates() {
    assert!(
        STACK.get_file("stack.toml").is_some(),
        "the stack has no manifest, so this binary has nothing to operate"
    );
}

/// The app is a built tree, not the repository that builds it.
///
/// `built-<tag>` holds the output; `main` holds the source. Pinning the wrong one
/// would embed a `package.json` and no `index.html`, which the check above would
/// catch — this says which mistake it was.
#[test]
fn what_is_carried_is_the_built_tree_rather_than_its_source() {
    assert!(
        APP.get_file("package.json").is_none(),
        "this is lemonfiber-web's source, not the tree its publish step builds"
    );
}
