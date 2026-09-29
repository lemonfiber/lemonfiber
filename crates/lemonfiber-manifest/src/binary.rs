//! Whether the running binary is one a stack says it can be run by.
//!
//! A stack names the oldest `lemonfiber` it works with in `min_cli_version`, because a
//! stack can come to rely on something a binary learned after the stack's schema
//! generation was fixed: a generation says how the file is read, and this says what the
//! binary reading it has to be able to do.

use std::cmp::Ordering;

use crate::{Failure, Manifest};

impl Manifest {
    /// Whether a binary at `running` may run this stack.
    ///
    /// Nothing is required where the stack names nothing, and nothing where either
    /// version is not one this can place: an ordering cannot be claimed between two
    /// strings that are not versions, and the schema generation is the gate that
    /// refuses a file this build cannot read at all.
    ///
    /// # Errors
    ///
    /// [`Failure::BinaryTooOld`] where the stack names a version newer than `running`.
    pub fn admits(&self, running: &str) -> Result<(), Failure> {
        let (Some(required), Some(this)) =
            (Version::read(&self.min_cli_version), Version::read(running))
        else {
            return Ok(());
        };
        if this < required {
            return Err(Failure::BinaryTooOld {
                required: self.min_cli_version.trim().to_owned(),
                running: running.trim().to_owned(),
            });
        }
        Ok(())
    }
}

/// A version as `major.minor.patch`, with the pre-release that follows a hyphen.
#[derive(Debug, PartialEq, Eq)]
struct Version {
    /// The three numbers, compared in order.
    release: [u64; 3],
    /// What follows the hyphen, where anything does.
    pre: Option<String>,
}

impl Version {
    /// A version as written, or nothing where it is not one.
    ///
    /// Build metadata after a `+` is dropped, because it does not order versions.
    fn read(text: &str) -> Option<Self> {
        let written = text.trim().trim_start_matches('v');
        let written = written
            .split_once('+')
            .map_or(written, |(version, _)| version);
        let (numbers, pre) = match written.split_once('-') {
            Some((numbers, pre)) => (numbers, Some(pre.to_owned())),
            None => (written, None),
        };
        let mut parts = numbers.split('.').map(|part| part.parse::<u64>().ok());
        let release = [parts.next()??, parts.next()??, parts.next()??];
        parts.next().is_none().then_some(Self { release, pre })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    /// The numbers first; then a release comes after every pre-release of it, and two
    /// pre-releases are ordered as their text is.
    fn cmp(&self, other: &Self) -> Ordering {
        self.release
            .cmp(&other.release)
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(mine), Some(theirs)) => mine.cmp(theirs),
            })
    }
}

#[cfg(test)]
mod tests;
