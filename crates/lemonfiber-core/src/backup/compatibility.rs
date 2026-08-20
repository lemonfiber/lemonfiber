//! Whether an archive restores into this installation at all.
//!
//! Two questions, asked before anything is unpacked: was the archive written by a
//! lemonfiber this one can still read, and was it rooted somewhere else. Both are
//! answered from the manifest alone, so a refusal costs nothing and an operator
//! learns why before any file is touched.

use std::path::Path;

use super::Manifest;

/// A three-part version, compared to decide whether an archive restores here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Version {
    pub(crate) major: u32,
    pub(crate) minor: u32,
    pub(crate) patch: u32,
}

impl Version {
    /// Parse a `major.minor.patch` string, ignoring any pre-release suffix, and
    /// return nothing for anything that is not three numbers.
    ///
    /// Lenient about a trailing `-rc.1` because a release candidate restores like
    /// the release it precedes; strict about the three numbers because a version
    /// that cannot be read is a corrupt archive, not a guess to make.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        // `split` always yields at least the whole string, so a version with no
        // `-` suffix is its own first segment — `unwrap_or(text)` says that plainly.
        let core = text.split('-').next().unwrap_or(text);
        let mut parts = core.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

/// Whether an archive read back can be restored by the running build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compatibility {
    /// The versions and schema agree; restore may proceed.
    Compatible,
    /// The archive was written by a newer lemonfiber; refuse, stating the gap.
    TooNew {
        /// The version that wrote the archive.
        archive: String,
        /// The version being restored with.
        current: String,
    },
    /// The archive is at least a whole major version behind; attempt with a
    /// warning.
    Downgrade {
        /// The version that wrote the archive.
        archive: String,
        /// The version being restored with.
        current: String,
    },
    /// The archive's format, or its stated version, cannot be restored safely.
    Incompatible {
        /// Why it cannot be restored.
        detail: String,
    },
}

impl Compatibility {
    /// Decide whether `manifest` can be restored by the running build.
    ///
    /// The schema is checked first: an archive laid out in a format this build
    /// does not write is refused before its version is even considered, because a
    /// format it cannot read is one it cannot restore. Then the versions: a newer
    /// archive is refused with the gap named, since it may hold state this build
    /// would corrupt; an archive a whole major version behind is allowed but
    /// warned about; anything else — same major, older or level — is compatible.
    #[must_use]
    pub fn assess(manifest: &Manifest, current_version: &str, current_schema: u32) -> Self {
        if manifest.schema != current_schema {
            return Self::Incompatible {
                detail: format!(
                    "the archive is format {} and this lemonfiber reads format {current_schema}",
                    manifest.schema
                ),
            };
        }

        let (Some(archive), Some(current)) = (
            Version::parse(&manifest.product_version),
            Version::parse(current_version),
        ) else {
            return Self::Incompatible {
                detail: format!(
                    "the archive states version {:?}, which cannot be read",
                    manifest.product_version
                ),
            };
        };

        if archive > current {
            Self::TooNew {
                archive: manifest.product_version.clone(),
                current: current_version.to_owned(),
            }
        } else if archive.major < current.major {
            Self::Downgrade {
                archive: manifest.product_version.clone(),
                current: current_version.to_owned(),
            }
        } else {
            Self::Compatible
        }
    }
}

/// A restore whose archive was taken against a different data root than the one
/// configured now, so its stored paths would land where nothing exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relocation {
    /// The data root the archive was taken against.
    pub was: String,
    /// The data root configured now.
    pub now: String,
}

/// Notice a restore to a different data root than the archive was taken against.
///
/// The difference is surfaced rather than silently followed, because restoring an
/// archive's recorded paths onto a machine whose library lives elsewhere would
/// recreate directories that point at nothing — the operator is offered the chance
/// to re-point instead.
#[must_use]
pub fn relocation(manifest: &Manifest, current_root: &Path) -> Option<Relocation> {
    // Compared as paths, not as strings: `/srv/media` and `/srv/media/` are the
    // same root, and a cosmetic difference must not raise a re-point the operator
    // then has to dismiss. Path equality is by component, so a trailing separator
    // and other spellings of the same place fall together.
    if Path::new(&manifest.data_root) == current_root {
        None
    } else {
        Some(Relocation {
            was: manifest.data_root.clone(),
            now: current_root.to_string_lossy().into_owned(),
        })
    }
}
