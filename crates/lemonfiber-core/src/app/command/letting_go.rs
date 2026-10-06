//! Letting one completed download go, files and all.
//!
//! Its own file beside the command that carries it, the way every other value a
//! command carries has one.

/// Which completed download to let go, and the offer that agreed to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LettingGo {
    /// Which completed download, by the name the client and the account both use.
    pub download: String,
    /// The offer being answered, as the run that made it named itself; without
    /// one, the cost is stated and nothing is removed.
    pub agreement: Option<String>,
}
