//! Writing what the log viewer shows into a file of its own.

/// What the log viewer is showing, to be redacted and written beside where it was run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exporting {
    /// The lines as the viewer shows them, one per line.
    pub text: String,
    /// How many exports this viewer has asked for, this one included.
    pub written: usize,
}
