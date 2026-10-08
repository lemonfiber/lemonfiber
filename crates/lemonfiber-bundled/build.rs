//! Assemble the embedded stack's manifest into one text, or refuse to build.
//!
//! The manifest is a root and a file per service, and what reads it
//! at compile time — the vocabulary the bundled services publish, the names a
//! plugin may not take — wants it as the one text a reader parses. A stack whose
//! files break the rules about them is refused here, while it is still a compile
//! error, by the assembly the binary uses at run time.

use std::path::{Path, PathBuf};
use std::process::exit;

/// Where the embedded stack lives, from this crate.
const STACK: &str = "../../assets/media-stack";

fn main() {
    let stack = Path::new(env!("CARGO_MANIFEST_DIR")).join(STACK);
    println!(
        "cargo::rerun-if-changed={}",
        stack.join(lemonfiber_manifest::assembly::ROOT).display()
    );
    println!(
        "cargo::rerun-if-changed={}",
        stack
            .join(lemonfiber_manifest::assembly::SERVICES)
            .display()
    );

    let text = lemonfiber_manifest::read(&stack).unwrap_or_else(|refused| {
        eprintln!("error: the embedded stack's manifest cannot be assembled\n\n{refused}\n");
        eprintln!("Stack: {}", stack.display());
        eprintln!("If the directory is empty, the submodule is not populated:");
        eprintln!("  git submodule update --init --recursive");
        exit(1);
    });

    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default());
    if let Err(unwritten) = std::fs::write(out.join("stack.toml"), text) {
        eprintln!("error: the assembled manifest could not be written: {unwritten}");
        exit(1);
    }
}
