//! What this binary carries inside itself: the stack and the web app, both read at
//! compile time, so neither can fail at run time.

use include_dir::{include_dir, Dir};

/// The stack this binary carries.
///
/// Embedding it means the common install has one thing to fetch rather than
/// two, and `build.rs` has already refused to produce this binary if the
/// manifest is one it could not read.
pub static STACK: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");

/// The app this binary serves a browser.
///
/// It arrives as a pinned submodule at `assets/web`, embedded exactly as the
/// stack above it is — the built tree of a `lemonfiber-web` tag rather than its
/// source, so what is carried is addressable as a git revision.
///
/// A checkout whose submodule is not populated carries an empty directory rather
/// than failing, so the repository can be worked in without it. What cannot
/// happen is carrying an app that speaks a wire version this binary does not
/// serve: `build.rs` compares the two and refuses the build.
pub static APP: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../assets/web");
