//! A file that could not be read as the shape it should hold.

use std::fmt;

/// A file that could not be read as the shape it should hold, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable {
    /// Which file, by the name it has in the configuration directory.
    pub file: &'static str,
    /// What was wrong with it.
    pub why: String,
}

impl fmt::Display for Unreadable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} could not be read: {}", self.file, self.why)
    }
}

impl std::error::Error for Unreadable {}
