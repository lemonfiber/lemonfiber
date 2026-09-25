//! The committed surface is the one this build emits, and it never loses anything.
//!
//! [`lemonfiber_api::contract::stability`] explains why the surface exists and what
//! counts as breaking it. What it did not have until now is anything that runs: the
//! guard was asked by the generator and by nothing else, which makes it a guard
//! against the person who runs `just surface` and no guard at all against the person
//! who does not. A removal landed by somebody who never ran it would be invisible
//! until a release carried it and somebody's script stopped parsing.
//!
//! So the two halves are asserted here, where the suite runs them on every change.
//! They are deliberately two rules rather than one comparison, because they fail for
//! different reasons and want different answers: a surface that lost something is a
//! decision to make about the wire version, and a surface that merely moved on is a
//! file to regenerate.

use std::path::{Path, PathBuf};

use lemonfiber_api::contract::stability::{rendered, Surface, SURFACE_PATH};
use lemonfiber_api::contract::Contract;

/// Where the committed surface lives, from this crate.
fn committed_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(SURFACE_PATH)
}

/// The surface as it is checked in.
fn committed() -> Surface {
    let Ok(text) = std::fs::read_to_string(committed_path()) else {
        unreachable!("the committed surface is part of this repository");
    };
    let Some(surface) = Surface::parse(&text) else {
        unreachable!("the committed surface is this program's own output");
    };
    surface
}

/// Nothing a consumer could be reading has gone, under an unchanged wire version.
///
/// The whole promise the version number carries, checked the way the generator
/// checks it. Failing here is not a file to regenerate: either the field goes back,
/// or `API_VERSION` moves and consumers are told the shapes changed under them.
#[test]
fn this_build_still_describes_everything_the_committed_surface_does() {
    let breaks = Surface::broken(&committed(), &Surface::of(&Contract::describe()));

    assert!(
        breaks.is_empty(),
        "this build drops what the committed surface describes, under an unchanged \
         api_version:\n{}\nEither put them back, or increment API_VERSION first.",
        rendered(&breaks)
    );
}

/// And the committed surface is not merely unbroken but current.
///
/// An addition breaks nothing, which is exactly why it needs its own rule: without
/// one, a field added and never recorded would sit outside the surface indefinitely,
/// and the day somebody removed it again the rule above would have nothing to notice
/// it against. Run `just surface` — it is the same program, writing rather than
/// comparing.
#[test]
fn the_committed_surface_is_the_one_this_build_would_write() {
    let Some(fresh) = Surface::of(&Contract::describe()).to_json() else {
        unreachable!("a surface this build built is a surface this build can render");
    };
    let Ok(text) = std::fs::read_to_string(committed_path()) else {
        unreachable!("the committed surface is part of this repository");
    };

    assert_eq!(
        text.trim_end(),
        fresh.trim_end(),
        "the committed surface is out of date — run `just surface`"
    );
}
