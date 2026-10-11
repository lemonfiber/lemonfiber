//! The default bundle's file, `bundle/bundle.toml` in `lemonfiber-plugins`, read and
//! held to its rules, with every copy beside it held to its pin.
//!
//! This crate is inert, as `lemonfiber-manifest` is: it reads the bundle into types and
//! refuses one it cannot read. The core compiles what it reads in, so a pin a copy does
//! not match is a build that fails rather than a plugin trusted at a manifest nobody
//! pinned.

mod copies;
mod parts;
mod rules;
mod tree;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use lemonfiber_manifest::{Form, Profile, Removed, Wiring};
use serde::Deserialize;

pub use tree::digest as tree_digest;

/// The bundle's file, from the root of `lemonfiber-plugins`.
pub const FILE: &str = "bundle/bundle.toml";

/// The directory each pinned plugin's copy is held in, from the same root.
pub const COPIES: &str = "bundle/plugins";

/// What a schema 1 copy's file name ends with, after the plugin's id.
pub const EXTENSION: &str = ".toml";

/// The manifest at the root of a schema 2 copy.
pub const MANIFEST: &str = "plugin.toml";

/// The schema versions this crate reads: a bundle of plugins alone, and one that also
/// declares the stack's own tables and pins each plugin by its tree.
pub const SCHEMAS: [u32; 2] = [1, 2];

/// What a pinned digest starts with.
pub const DIGEST_PREFIX: &str = "sha256:";

/// The bundle's file, as written.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    /// The schema version the file is written in.
    pub schema: u32,
    /// Every profile of the stack it ships, at schema 2.
    #[serde(default)]
    pub profile: Vec<Profile>,
    /// Every form of the stack it ships, at schema 2.
    #[serde(default)]
    pub form: Vec<Form>,
    /// The stack's choices of filler and its links by name, at schema 2.
    #[serde(default)]
    pub wiring: Vec<Wiring>,
    /// Every service the catalogue no longer carries, at schema 2.
    #[serde(default)]
    pub removed: Vec<Removed>,
    /// Every first-party plugin, ordered by id.
    #[serde(default)]
    pub plugin: Vec<Pin>,
}

/// Whether the bundle assembles a part into the stack it ships or installs it through the
/// plugin path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Its one service is a stack service, its in-process adapter included.
    Assembled,
    /// Its adapter has shipped and it moves in place of the service it replaces.
    Installed,
}

/// One first-party plugin, as the bundle pins it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    /// The plugin's `[plugin].id`.
    pub id: String,
    /// The plugin's repository, as an https URL.
    pub origin: String,
    /// The version the release train tagged it at, without the `v`; absent at schema 2
    /// for a plugin pinned at a commit on its main branch.
    #[serde(default)]
    pub release: Option<String>,
    /// The full commit of its repository the copy was taken from.
    pub revision: String,
    /// The digest of its manifest at that revision, written `sha256:<64 hex>`.
    pub manifest: String,
    /// The digest of its tree at that revision, written `sha256:<64 hex>`, at schema 2.
    #[serde(default)]
    pub tree: Option<String>,
    /// The capabilities it fills by default.
    pub fills: Vec<String>,
    /// Whether it is assembled or installed, at schema 2.
    #[serde(default)]
    pub stage: Option<Stage>,
    /// The profile its services sit in, at schema 2.
    #[serde(default)]
    pub profile: Option<String>,
    /// The day its upstream last released, `YYYY-MM-DD`, at schema 2.
    #[serde(default)]
    pub last_release: Option<String>,
}

/// One pinned plugin with the copy the bundle holds of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// The pin.
    pub pin: Pin,
    /// Its manifest's text, byte for byte.
    pub copy: String,
    /// Every file of its tree beside the manifest, by its path from the copy's root;
    /// empty at schema 1.
    pub tree: BTreeMap<String, Vec<u8>>,
}

impl Held {
    /// The SHA-256 of the manifest, in lower-case hexadecimal, as the core records an
    /// installed manifest's.
    #[must_use]
    pub fn digest(&self) -> String {
        hex(self.copy.as_bytes())
    }
}

/// Why a bundle could not be read.
#[derive(Debug, thiserror::Error)]
pub enum Refused {
    /// A file could not be read.
    #[error("{path}: {source}")]
    Unread {
        /// The file.
        path: String,
        /// What reading it answered.
        source: std::io::Error,
    },
    /// The bundle's file is not one this crate reads.
    #[error("{FILE}: {0}")]
    Syntax(#[from] toml::de::Error),
    /// The bundle breaks its rules, every one named.
    #[error("{FILE} breaks its rules:\n{}", .0.join("\n"))]
    Rules(Vec<String>),
}

/// The bundle in the catalogue checked out at `root`, with every copy it pins.
///
/// # Errors
///
/// [`Refused`] where a file cannot be read or is reached through a symbolic link, where a
/// name is not UTF-8, where the bundle is not TOML of this shape, and where it breaks a
/// rule: a schema this crate does not read, plugins out of order or named twice, a pin
/// malformed, a copy missing, unpinned, naming another plugin, holding two paths that
/// differ only in case or not matching its digests, or a stack table naming what no
/// pinned plugin declares.
pub fn read(root: &Path) -> Result<(Bundle, Vec<Held>), Refused> {
    let linked: BTreeSet<String> = [FILE, COPIES]
        .iter()
        .filter_map(|relative| copies::linked(root, relative))
        .map(|at| format!("{at} is a symbolic link, which nothing the bundle reads may be"))
        .collect();
    if !linked.is_empty() {
        return Err(Refused::Rules(linked.into_iter().collect()));
    }
    let text = copies::text(&root.join(FILE))?;
    let bundle: Bundle = toml::from_str(&text)?;
    let copies = copies::read(&root.join(COPIES), bundle.schema)?;
    let mut broken = rules::broken(&bundle, &copies);
    if !broken.is_empty() {
        broken.sort();
        broken.dedup();
        return Err(Refused::Rules(broken));
    }
    let mut copies = copies;
    let held = bundle
        .plugin
        .iter()
        .map(|pin| {
            let copy = copies.remove(&pin.id).unwrap_or_default();
            Held {
                pin: pin.clone(),
                copy: copy.manifest.unwrap_or_default(),
                tree: copy.files,
            }
        })
        .collect();
    Ok((bundle, held))
}

/// The SHA-256 of `bytes`, in lower-case hexadecimal.
fn hex(bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    let mut rendered = String::with_capacity(digest.as_ref().len() * 2);
    for byte in digest.as_ref() {
        let _ = write!(rendered, "{byte:02x}");
    }
    rendered
}

#[cfg(test)]
mod tests;
