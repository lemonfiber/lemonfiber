//! What redeeming or releasing the name of work that ended before it finished answers with.

use serde::Serialize;

/// Why work ended before it finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "JobEnding")]
pub enum Ending {
    /// Its name was released.
    Released,
    /// Nothing asked about it for a whole lease, so it was let go.
    LetGo,
    /// The services it followed stopped saying anything.
    Quiet,
}

/// Work that ended before it finished: its name, what it was, and why it ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Ended {
    /// The name the work was answered with.
    pub job: String,
    /// The action that was asked for, as it was named.
    pub action: String,
    /// Why it ended.
    pub ended: Ending,
}
