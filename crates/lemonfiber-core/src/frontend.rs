//! Where the frontend comes from, and finding one file in it.
//!
//! The same shape as [`crate::stack`], for the same reason: the app ships inside
//! the binary so the common install has one thing to fetch and nothing to go
//! stale, and an operator building their own points at a directory instead.
//! Everything above this module stops being able to tell the difference.
//!
//! A path in, bytes out. There is no server here and there must not be one —
//! this crate cannot render and cannot listen, which is what keeps a surface a
//! rendering rather than a capability. What a browser is told a file *is*, and
//! which routes reach this at all, belong to whatever does the serving.
//!
//! Two decisions live here rather than in the surface, because both are about
//! the app rather than about HTTP. A path that climbs out of the directory is
//! refused instead of resolved. And a path naming no file at all is the app
//! itself, since its own router reads the path once the page is loaded.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use include_dir::Dir;

/// The one file every built app has, and the answer to a path that names none.
const INDEX: &str = "index.html";

/// Where the frontend lemonfiber serves is read from.
#[derive(Debug, Clone, Copy)]
pub enum Source {
    /// The app compiled into this binary.
    Embedded(&'static Dir<'static>),
    /// A directory on disk, named by whoever is running lemonfiber.
    External(&'static Path),
}

/// One file of the app: what it is called, and what is in it.
///
/// The path travels with the bytes because it is what says how to read them. A
/// caller handed bytes alone would have to be told their type separately, and
/// two arguments that must agree eventually do not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    /// Where this file sits within the app.
    pub path: PathBuf,
    /// The file itself, borrowed from the binary or read from disk.
    pub bytes: Cow<'static, [u8]>,
}

impl Source {
    /// The file a path asks for, or the app itself where it asks for no file.
    ///
    /// A path with an extension names a file: if it is not here, nothing is, and
    /// answering with the page instead would hand a script that asked for a
    /// stylesheet a document it cannot use. A path without one is a route the
    /// app's own router will read, so the app is the honest answer to it.
    #[must_use]
    pub fn asset(self, asked: &str) -> Option<Asset> {
        let within = within(asked)?;
        if let Some(asset) = self.file(&within) {
            return Some(asset);
        }
        if within.extension().is_some() {
            return None;
        }
        self.file(Path::new(INDEX))
    }

    /// Whether there is an app here at all.
    ///
    /// Asked before anything is served rather than discovered one missing file at
    /// a time, so a build carrying no app can say so once instead of answering
    /// every request with the same absence.
    #[must_use]
    pub fn holds_an_app(self) -> bool {
        self.file(Path::new(INDEX)).is_some()
    }

    /// One file, exactly as named, however this app is stored.
    fn file(self, within: &Path) -> Option<Asset> {
        let bytes = match self {
            Self::Embedded(dir) => Cow::Borrowed(dir.get_file(within)?.contents()),
            // A file that cannot be read is a file that is not here. Whether this
            // is an app at all was settled before anything was served, and a
            // permission fault on one asset is not a different answer to a caller.
            Self::External(root) => Cow::Owned(std::fs::read(root.join(within)).ok()?),
        };
        Some(Asset {
            path: within.to_path_buf(),
            bytes,
        })
    }
}

/// Where a path lands within the app, or nothing where it leads outside it.
///
/// The rule about what a supplied name may reach is [`crate::within`]'s, shared
/// with the other caller that turns request text into a path beneath a directory
/// lemonfiber chose. What is decided here is only what naming nothing means, which
/// is about the app rather than about paths: a path with no file in it is a route
/// the app's own router reads once the page is loaded, so the app is the answer.
fn within(asked: &str) -> Option<PathBuf> {
    let path = crate::within::beneath(asked)?;
    if path.as_os_str().is_empty() {
        return Some(PathBuf::from(INDEX));
    }
    Some(path)
}

#[cfg(test)]
mod tests;
