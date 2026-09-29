//! Where a plugin to install is: a directory on this machine, or a git repository.
//!
//! Told apart by how the operator wrote it, and by nothing that has to be asked: an
//! address with a scheme git speaks, or git's own `user@host:path` form, is a
//! repository, and anything else is a path. A path that happens to look like neither
//! is still a path, and is refused as one if nothing is there.
//!
//! A repository may name a revision after its last `@` — a branch, a tag or a whole
//! commit — and names none otherwise, which means whatever it serves as its default.
//! Either way what is installed is the one commit that resolves to, and that commit is
//! what the record keeps.

use std::path::PathBuf;

/// The beginnings of an address git fetches from.
const GIT: &[&str] = &["https://", "http://", "ssh://", "git://", "git@"];

/// Where the plugin to install is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
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

#[cfg(test)]
mod tests;
