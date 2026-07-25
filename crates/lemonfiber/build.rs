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
