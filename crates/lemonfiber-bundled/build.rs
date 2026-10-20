//! Assemble the stack the default bundle declares into one manifest text and its pins
//! into Rust, or refuse to build.
//!
//! The bundle is read by `lemonfiber-bundle`, which holds every copy to its pin. Each
//! part it assembles becomes a stack service; each part it installs is kept as the
//! media stack it replaces declares it, until the part moves. What reads the stack at
//! compile time — the vocabulary the bundled services publish, the names a plugin may
//! not take — reads that text, and a stack the manifest reader refuses is refused
//! here, while it is still a compile error. The pins and the first-party set the core
//! trusts are written out beside it, so neither is typed by hand.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::exit;

/// Where the media stack the bundle replaces lives, from this crate.
const STACK: &str = "../../assets/media-stack";

/// Where the catalogue holding the default bundle lives, from this crate.
const CATALOGUE: &str = "../../assets/plugins";

fn main() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default());
    let catalogue = Path::new(env!("CARGO_MANIFEST_DIR")).join(CATALOGUE);
    let held = bundle(&catalogue, &out);
    stack(&catalogue, &held, &out);
}

/// The stack the bundle at `catalogue` declares, with each part it installs kept as the
/// media stack declares it, assembled into `stack.toml` in `out`.
fn stack(catalogue: &Path, held: &[lemonfiber_bundle::Held], out: &Path) {
    let media = Path::new(env!("CARGO_MANIFEST_DIR")).join(STACK);
    for path in [
        media.join(lemonfiber_manifest::assembly::ROOT),
        media.join(lemonfiber_manifest::assembly::SERVICES),
    ] {
        println!("cargo::rerun-if-changed={}", path.display());
    }
    let frozen = lemonfiber_manifest::read(&media).unwrap_or_else(|refused| {
        refuse(
            "the media stack cannot be assembled",
            &refused.to_string(),
            &media,
        )
    });
    let declared =
        std::fs::read_to_string(catalogue.join(lemonfiber_bundle::FILE)).unwrap_or_else(|unread| {
            refuse("the bundle cannot be read", &unread.to_string(), catalogue)
        });
    let text = lemonfiber_bundle::assembled(&declared, held, env!("CARGO_PKG_VERSION"))
        .and_then(|assembled| lemonfiber_bundle::kept(&assembled, &frozen, held))
        .unwrap_or_else(|refused| {
            refuse(
                "the bundle's stack cannot be assembled",
                &refused.join("\n"),
                catalogue,
            )
        });
    if let Err(unreadable) = lemonfiber_manifest::Manifest::from_toml(&text) {
        refuse(
            "the bundle's stack is not one lemonfiber reads",
            &unreadable.to_string(),
            catalogue,
        );
    }
    if let Err(unwritten) = std::fs::write(out.join("stack.toml"), text) {
        eprintln!("error: the assembled manifest could not be written: {unwritten}");
        exit(1);
    }
}

/// Stop the build, saying `what` and why, at `at`.
fn refuse(what: &str, why: &str, at: &Path) -> ! {
    eprintln!("error: {what}\n\n{why}\n");
    eprintln!("At: {}", at.display());
    eprintln!("If the directory is empty, the submodule is not populated:");
    eprintln!("  git submodule update --init --recursive");
    exit(1);
}

/// The default bundle's copies, held to their pins, with the pins written as Rust into
/// `bundle.rs` in `out`.
fn bundle(catalogue: &Path, out: &Path) -> Vec<lemonfiber_bundle::Held> {
    println!(
        "cargo::rerun-if-changed={}",
        catalogue.join(lemonfiber_bundle::FILE).display()
    );
    println!(
        "cargo::rerun-if-changed={}",
        catalogue.join(lemonfiber_bundle::COPIES).display()
    );

    let (_, held) = lemonfiber_bundle::read(catalogue).unwrap_or_else(|refused| {
        refuse(
            "the embedded bundle cannot be read",
            &refused.to_string(),
            catalogue,
        )
    });

    let copies = catalogue.join(lemonfiber_bundle::COPIES);
    if let Err(unwritten) = std::fs::write(out.join("bundle.rs"), rust(&held, &copies)) {
        eprintln!("error: the bundle's pins could not be written: {unwritten}");
        exit(1);
    }
    held
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
