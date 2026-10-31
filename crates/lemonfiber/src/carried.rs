//! What this binary carries inside itself: the stack and the web app, both read at
//! compile time, so neither can fail at run time.

use include_dir::{include_dir, Dir};

/// The stack this binary carries.
///
/// Embedding it means the common install has one thing to fetch rather than
/// two, and `lemonfiber-bundled` has already refused to build if the manifest is one
/// it could not read. What is embedded is what runs of it, so an operator's stack
/// directory is written with the stack's starting files and not with the CI, scripts and
/// hooks of the repositories it comes from, and never with its manifest.
pub use lemonfiber_core::stack::BUNDLED as STACK;

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
