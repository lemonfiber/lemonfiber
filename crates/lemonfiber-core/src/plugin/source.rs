//! Where a plugin to install is: a name in the catalogue, a directory on this machine,
//! or a git repository.
//!
//! Told apart by how the operator wrote it, and by nothing that has to be asked: an
//! address with a scheme git speaks, or git's own `user@host:path` form, is a
//! repository; a bare word shaped as a plugin's id is a name; and anything else is a
//! path. A directory in the current one whose name is shaped as an id is written with
//! `./` in front of it. A path that happens to look like none of these is still a path,
//! and is refused as one if nothing is there.
//!
//! A repository is fetched over https and nothing else: an address beginning with any
//! other scheme git reads is still a repository, and [`unspoken`] names that scheme so
//! the install is refused for it rather than for a path that is not there.
//!
//! A repository may name a revision after its last `@` — a branch, a tag or a whole
//! commit — and names none otherwise, which means whatever it serves as its default.
//! Either way what is installed is the one commit that resolves to, and that commit is
//! what the record keeps.

use std::path::PathBuf;

/// The beginnings of an address git fetches from.
const GIT: &[&str] = &[SPOKEN, "http://", "ssh://", "git://", "git@"];

/// The one transport a repository is fetched over. Every other beginning git reads as
/// a repository is still read as one, so it is refused by name rather than looked for
/// as a path.
pub const SPOKEN: &str = "https://";

/// Where the plugin to install is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A plugin's id, resolved through the catalogue's signed index.
    Name(String),
    /// On this machine: its directory, or the `plugin.toml` inside it.
    Path(PathBuf),
    /// A git repository, and the revision named where one was.
    Git {
        /// The repository, as it is fetched from.
        url: String,
        /// The branch, tag or commit named after the last `@`, where one was.
        revision: Option<String>,
    },
}

impl Source {
    /// A source as the operator wrote it.
    #[must_use]
    pub fn named(written: &str) -> Self {
        if !GIT.iter().any(|scheme| written.starts_with(scheme)) {
            if is_id(written) {
                return Self::Name(written.to_owned());
            }
            return Self::Path(PathBuf::from(written));
        }
        // The revision is looked for only in the last step of the path, so neither the
        // `user@` of an address nor git's own `git@host:` is read as one.
        let path_from = written.find("://").map_or_else(
            || written.find(':').map_or(written.len(), |colon| colon + 1),
            |scheme| scheme + 3,
        );
        let last_step = written[path_from..]
            .rfind('/')
            .map_or(path_from, |slash| path_from + slash);
        match written[last_step..].rfind('@') {
            Some(at) if last_step + at + 1 < written.len() => Self::Git {
                url: written[..last_step + at].to_owned(),
                revision: Some(written[last_step + at + 1..].to_owned()),
            },
            _ => Self::Git {
                url: written.to_owned(),
                revision: None,
            },
        }
    }

    /// Whether this is a repository rather than a directory.
    #[must_use]
    pub const fn is_git(&self) -> bool {
        matches!(self, Self::Git { .. })
    }
}

/// The beginning a repository's address is written with, where it is one git reads and
/// not [`SPOKEN`].
#[must_use]
pub fn unspoken(url: &str) -> Option<&'static str> {
    GIT.iter()
        .copied()
        .filter(|scheme| *scheme != SPOKEN)
        .find(|scheme| url.starts_with(scheme))
}

/// Whether a word is shaped as a plugin's id: lowercase letters and digits in words
/// joined by single hyphens, beginning with a letter.
///
/// The shape the catalogue registers an id under, so a word of this shape is one the
/// catalogue could hold and anything else could not.
fn is_id(written: &str) -> bool {
    written.starts_with(|letter: char| letter.is_ascii_lowercase())
        && written.split('-').all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests;
