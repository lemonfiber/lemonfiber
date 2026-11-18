//! Reading what the services wrote to their logs.

/// Which logs to read, how far back, and whether to go on reading as they are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingLogs {
    /// The forms whose services to read, or none for every service.
    pub forms: Vec<String>,
    /// The services to read, narrowing the forms, or none for all of theirs.
    pub services: Vec<String>,
    /// How many lines of each service's scrollback to begin with.
    pub tail: u32,
    /// Whether to go on reading lines as they are written, until the stream ends.
    pub follow: bool,
}
