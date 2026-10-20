//! The stack and the default bundle this build embeds.
//!
//! `assets/plugins` is the catalogue whose `bundle/bundle.toml` pins every first-party
//! plugin. Its pins and their manifests are compiled in from there, and the build fails
//! when a copy is not the manifest its pin names.
//!
//! The stack is the one that bundle declares: its own tables, a service for each part
//! it assembles, and each part it installs as `assets/media-stack` declares it until
//! the part moves. Everything compiled against the bundled stack reads it through here,
//! and the build fails before any of it compiles when the stack is not one lemonfiber
//! reads.

/// The embedded stack's manifest: the bundle's tables, then a service for each part, the
/// assembled ones in the bundle's order and the installed ones after them.
pub const STACK: &str = include_str!(concat!(env!("OUT_DIR"), "/stack.toml"));

/// One first-party plugin, as the signed default bundle names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirstParty {
    /// Its id.
    pub plugin: &'static str,
    /// The SHA-256 of its manifest, in lower-case hexadecimal.
    pub manifest: &'static str,
    /// The digest of its tree, `sha256:<hex>`; empty where the bundle pins none.
    pub tree: &'static str,
}

/// One first-party plugin as the bundle pins it, with its manifest and, where the bundle
/// installs it, its tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pinned {
    /// Its id and the digests of its manifest and its tree.
    pub first_party: FirstParty,
    /// Its repository, as an https URL.
    pub origin: &'static str,
    /// The version the release train tagged it at, where it was tagged.
    pub release: Option<&'static str>,
    /// The commit of its repository the manifest was copied from.
    pub revision: &'static str,
    /// The capabilities it fills by default.
    pub fills: &'static [&'static str],
    /// Its manifest, byte for byte.
    pub copy: &'static str,
    /// Whether the bundle assembles it into the stack or installs it, where it says.
    pub stage: Option<Stage>,
    /// The profile its services sit in, where the bundle names one.
    pub profile: Option<&'static str>,
    /// Every file of its tree beside the manifest, by its path from the plugin's root,
    /// for a plugin the bundle installs; the stack carries an assembled one's.
    pub files: &'static [(&'static str, &'static [u8])],
}

/// Whether the bundle assembles a part into the stack or installs it through the plugin
/// path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Its one service is a stack service.
    Assembled,
    /// It is installed through the plugin path, in place of the service it replaces.
    Installed,
}

include!(concat!(env!("OUT_DIR"), "/bundle.rs"));
