//! The default bundle's file, `bundle/bundle.toml` in `lemonfiber-plugins`, read and
//! held to its rules, with every manifest copy beside it held to its pin.
//!
//! This crate is inert, as `lemonfiber-manifest` is: it reads the bundle into types and
//! refuses one it cannot read. The core compiles what it reads in, so a pin a copy does
//! not match is a build that fails rather than a plugin trusted at a manifest nobody
//! pinned.

mod rules;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The bundle's file, from the root of `lemonfiber-plugins`.
pub const FILE: &str = "bundle/bundle.toml";

/// The directory each pinned plugin's manifest is copied into, from the same root.
pub const COPIES: &str = "bundle/plugins";

/// What a copy's file name ends with, after the plugin's id.
pub const EXTENSION: &str = ".toml";

/// The one file in [`COPIES`] that is not a copy: what keeps an empty directory in git.
const KEEP: &str = ".gitkeep";

/// The schema version this crate reads.
pub const SCHEMA_VERSION: u32 = 1;

/// What a pinned manifest's digest starts with.
pub const DIGEST_PREFIX: &str = "sha256:";

/// The bundle's file, as written.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    /// The schema version the file is written in.
    pub schema: u32,
    /// Every first-party plugin, ordered by id.
    pub plugin: Vec<Pin>,
}

/// One first-party plugin, as the bundle pins it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    /// The plugin's `[plugin].id`.
    pub id: String,
    /// The plugin's repository, as an https URL.
    pub origin: String,
    /// The version the release train tagged it at, without the `v`.
    pub release: String,
    /// The full commit of its repository whose manifest names that release's image.
    pub revision: String,
    /// The digest of its manifest at that revision, written `sha256:<64 hex>`.
    pub manifest: String,
    /// The capabilities it fills by default.
    pub fills: Vec<String>,
}

/// One pinned plugin with the copy of its manifest the bundle holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// The pin.
    pub pin: Pin,
    /// The copy's text, which is the plugin's manifest byte for byte.
    pub copy: String,
}

impl Held {
    /// The SHA-256 of the copy, in lower-case hexadecimal, as the core records an
    /// installed manifest's.
    #[must_use]
    pub fn digest(&self) -> String {
        hex(&self.copy)
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
/// [`Refused`] where a file cannot be read, where the bundle is not TOML of this shape,
/// and where it breaks a rule: a schema this crate does not read, plugins out of order
/// or named twice, a pin malformed, a copy missing, unpinned, naming another plugin or
/// not matching its digest.
pub fn read(root: &Path) -> Result<Vec<Held>, Refused> {
    let text = readable(&root.join(FILE))?;
    let bundle: Bundle = toml::from_str(&text)?;
    let mut copies = copies(&root.join(COPIES))?;
    let mut broken = rules::broken(&bundle, &copies);
    if !broken.is_empty() {
        broken.sort();
        return Err(Refused::Rules(broken));
    }
    Ok(bundle
        .plugin
        .into_iter()
        .map(|pin| {
            let copy = copies.remove(&rules::file(&pin.id)).unwrap_or_default();
            Held { pin, copy }
        })
        .collect())
}

/// Every copy in `directory`, by its file name; a missing directory holds none.
fn copies(directory: &Path) -> Result<BTreeMap<String, String>, Refused> {
    let paths = match listed(directory) {
        Ok(paths) => paths,
        Err(missing) if missing.kind() == ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(source) => return Err(unread(directory, source)),
    };
    let mut copies = BTreeMap::new();
    for path in paths {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name == KEEP {
            continue;
        }
        copies.insert(name, readable(&path)?);
    }
    Ok(copies)
}

/// Every path in `directory`.
fn listed(directory: &Path) -> std::io::Result<Vec<PathBuf>> {
    std::fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect()
}

/// The text at `path`.
fn readable(path: &Path) -> Result<String, Refused> {
    std::fs::read_to_string(path).map_err(|source| unread(path, source))
}

/// The refusal for `path`, which reading answered with `source`.
fn unread(path: &Path, source: std::io::Error) -> Refused {
    Refused::Unread {
        path: path.display().to_string(),
        source,
    }
}

/// The SHA-256 of `text`, in lower-case hexadecimal.
fn hex(text: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    let mut rendered = String::with_capacity(digest.as_ref().len() * 2);
    for byte in digest.as_ref() {
        let _ = write!(rendered, "{byte:02x}");
    }
    rendered
}

#[cfg(test)]
mod tests;
