//! Assemble the embedded stack's manifest into one text and the default bundle's pins
//! into Rust, or refuse to build.
//!
//! The manifest is a root and a file per service, and what reads it
//! at compile time — the vocabulary the bundled services publish, the names a
//! plugin may not take — wants it as the one text a reader parses. A stack whose
//! files break the rules about them is refused here, while it is still a compile
//! error, by the assembly the binary uses at run time.
//!
//! The bundle is read by `lemonfiber-bundle`, which holds every copy to its pin, and
//! written out as the pins and the first-party set the core trusts, so neither is
//! typed by hand.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::exit;

/// Where the embedded stack lives, from this crate.
const STACK: &str = "../../assets/media-stack";

/// Where the catalogue holding the default bundle lives, from this crate.
const CATALOGUE: &str = "../../assets/plugins";

fn main() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default());
    stack(&out);
    bundle(&out);
}

/// The embedded stack's manifest, assembled into `stack.toml` in `out`.
fn stack(out: &Path) {
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

    if let Err(unwritten) = std::fs::write(out.join("stack.toml"), text) {
        eprintln!("error: the assembled manifest could not be written: {unwritten}");
        exit(1);
    }
}

/// The default bundle's pins, written as Rust into `bundle.rs` in `out`.
fn bundle(out: &Path) {
    let catalogue = Path::new(env!("CARGO_MANIFEST_DIR")).join(CATALOGUE);
    println!(
        "cargo::rerun-if-changed={}",
        catalogue.join(lemonfiber_bundle::FILE).display()
    );
    println!(
        "cargo::rerun-if-changed={}",
        catalogue.join(lemonfiber_bundle::COPIES).display()
    );

    let (_, held) = lemonfiber_bundle::read(&catalogue).unwrap_or_else(|refused| {
        eprintln!("error: the embedded bundle cannot be read\n\n{refused}\n");
        eprintln!("Catalogue: {}", catalogue.display());
        eprintln!("If the directory is empty, the submodule is not populated:");
        eprintln!("  git submodule update --init --recursive");
        exit(1);
    });

    let copies = catalogue.join(lemonfiber_bundle::COPIES);
    if let Err(unwritten) = std::fs::write(out.join("bundle.rs"), rust(&held, &copies)) {
        eprintln!("error: the bundle's pins could not be written: {unwritten}");
        exit(1);
    }
}

/// `held` as the two constants `lib.rs` includes, an installed plugin's tree included
/// from its copy under `copies`.
fn rust(held: &[lemonfiber_bundle::Held], copies: &Path) -> String {
    let mut pinned = String::new();
    let mut first_party = String::new();
    for one in held {
        let trusted = format!(
            "FirstParty {{ plugin: {:?}, manifest: {:?}, tree: {:?} }}",
            one.pin.id,
            one.digest(),
            one.pin.tree.as_deref().unwrap_or_default()
        );
        let _ = writeln!(first_party, "    {trusted},");
        let _ = writeln!(
            pinned,
            "    Pinned {{ first_party: {trusted}, origin: {:?}, release: {:?}, revision: {:?}, fills: &{:?}, copy: {:?}, stage: {}, profile: {:?}, files: &[{}] }},",
            one.pin.origin,
            one.pin.release,
            one.pin.revision,
            one.pin.fills,
            one.copy,
            staged(one.pin.stage),
            one.pin.profile,
            carried(one, copies)
        );
    }
    format!(
        "/// Every first-party plugin the bundle pins, with the copy of its manifest.\n\
         pub const PINNED: &[Pinned] = &[\n{pinned}];\n\n\
         /// Every first-party plugin this build embeds, by id and manifest digest.\n\
         pub const FIRST_PARTY: &[FirstParty] = &[\n{first_party}];\n"
    )
}

/// `stage` as the Rust that names it.
fn staged(stage: Option<lemonfiber_bundle::Stage>) -> &'static str {
    match stage {
        Some(lemonfiber_bundle::Stage::Assembled) => "Some(Stage::Assembled)",
        Some(lemonfiber_bundle::Stage::Installed) => "Some(Stage::Installed)",
        None => "None",
    }
}

/// Each file of an installed plugin's tree as a path and its bytes included from its
/// copy, nothing for one the stack assembles.
fn carried(one: &lemonfiber_bundle::Held, copies: &Path) -> String {
    if one.pin.stage != Some(lemonfiber_bundle::Stage::Installed) {
        return String::new();
    }
    let mut files = String::new();
    for path in one.tree.keys() {
        let at = copies.join(&one.pin.id).join(path);
        let _ = write!(
            files,
            "({path:?}, include_bytes!({:?})), ",
            at.display().to_string()
        );
    }
    files
}
