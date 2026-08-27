//! Refuse to build against a stack this binary could not operate.
//!
//! The stack is embedded, so a mismatched pairing is knowable at compile time —
//! and a build failure is the strongest available mitigation for the cost of
//! keeping the tool and the stack in separate repositories. An incompatible
//! pairing cannot ship, because it cannot compile.
//!
//! The same parser the binary uses at runtime does the checking, so the two
//! cannot disagree about what "readable" means.

use std::path::{Path, PathBuf};
use std::process::exit;

/// Where the embedded stack lives, from the workspace root.
const STACK: &str = "assets/media-stack";

/// Where the embedded app lives, from the workspace root.
const APP: &str = "assets/web";

/// What a built app says about itself, beside it.
const DECLARED: &str = "app.json";

/// The published statement of the wire version this binary speaks.
///
/// Compared against rather than the constant behind it, and the difference is the
/// point: the app declares the version its client took from this artefact, so this
/// compares like with like. What holds the artefact to the binary is its own test,
/// which regenerates it and fails on any difference — so a chain of two checks
/// covers what one could not reach from a build script.
const CONTRACT: &str = "contract/web-api.contract.json";

fn main() {
    let root = workspace_root().join(STACK);
    let manifest = root.join("stack.toml");

    println!("cargo::rerun-if-changed={}", manifest.display());

    let Ok(text) = std::fs::read_to_string(&manifest) else {
        refuse(&[
            &format!("the embedded stack is missing at {}", root.display()),
            "",
            "It is a git submodule, and a fresh clone does not populate it.",
            "",
            "  git submodule update --init --recursive",
        ]);
    };

    if let Err(err) = lemonfiber_manifest::Manifest::from_toml(&text) {
        refuse(&[
            "the embedded stack cannot be read by this build",
            "",
            &err.to_string(),
            "",
            &format!("Manifest: {}", manifest.display()),
            "The submodule and this binary are out of step; move the pin, or",
            "teach the parser the generation the stack now declares.",
        ]);
    }

    app_speaks_this_version();
}

/// Refuse to build against an app speaking a wire version this binary does not.
///
/// The same argument the stack above rests on, for the other embedded thing: a
/// mismatched pairing is knowable while it is still a compile error, and a build
/// failure is the strongest mitigation available for the cost of keeping the tool
/// and its interface in separate repositories.
///
/// **A build with no app is not a mismatch.** The submodule is absent from a clone
/// that has not been told to fetch it and from a build that deliberately carries no
/// app, and both are arrangements this product supports — the surface says there is
/// no app rather than answering with an empty document. So nothing here refuses an
/// absence; what it refuses is an app that is present and speaks something else.
fn app_speaks_this_version() {
    let root = workspace_root();
    let declared = root.join(APP).join(DECLARED);
    let contract = root.join(CONTRACT);

    println!("cargo::rerun-if-changed={}", declared.display());
    println!("cargo::rerun-if-changed={}", contract.display());

    let Ok(app) = std::fs::read_to_string(&declared) else {
        return;
    };

    let Some(speaks) = version_in(&app) else {
        refuse(&[
            "the embedded app does not say which wire version it speaks",
            "",
            &format!("Read: {}", declared.display()),
            "It is written by the app's own build. A tree without it is one that was",
            "assembled by hand rather than published, and this build cannot tell what",
            "it would be talking to.",
        ]);
    };

    let Some(serves) = std::fs::read_to_string(&contract)
        .as_deref()
        .ok()
        .and_then(version_in)
    else {
        refuse(&[
            "this build cannot read the wire version it serves",
            "",
            &format!("Read: {}", contract.display()),
            "The contract artefact is generated. Run `just contract`.",
        ]);
    };

    if speaks != serves {
        refuse(&[
            "the embedded app speaks a wire version this binary does not serve",
            "",
            &format!("The app says {speaks} and this binary serves {serves}."),
            "",
            &format!("App: {}", declared.display()),
            "Move the app pin to a build of the version this binary serves, or take",
            "the binary to the version the app speaks. Shipping the pair would put a",
            "browser in front of a server that answers it in another language.",
        ]);
    }
}

/// The wire version a generated document declares.
///
/// Both documents are written by a serialiser rather than by hand and both put the
/// field at the top level, so what is read is the first `"api_version"` either
/// carries. A hand-assembled file that nests one somewhere else is the case the
/// absence above refuses rather than one to be clever about.
fn version_in(text: &str) -> Option<u32> {
    let after = text.split_once("\"api_version\"")?.1;
    let digits: String = after
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// The workspace root, so paths in messages are the ones a reader recognises
/// rather than a walk back up out of this crate.
fn workspace_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(root) = crate_dir.parent().and_then(Path::parent) else {
        refuse(&["this crate is not two directories below a workspace root"]);
    };
    root.to_path_buf()
}

/// Stop the build, explaining what to do about it.
///
/// Written as `cargo::error` lines so the message survives Cargo's own framing
/// rather than being buried in a panic's backtrace.
fn refuse(lines: &[&str]) -> ! {
    for line in lines {
        println!("cargo::error={line}");
    }
    exit(1);
}
