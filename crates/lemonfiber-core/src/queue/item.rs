//! One thing in the pipeline, as the whole stack sees it.
//!
//! Deliberately not "a download" or "a queue record": the failure that matters
//! most is invisible inside either. An item that finished downloading and was
//! never imported is, to the client, a completed download; to the \*arr, nothing
//! at all. Both are content. Only a view that holds the two together sees that
//! something is wrong.
//!
//! So an item carries what each side said, and either side may be absent — which
//! is itself the signal in two of the categories.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// What the download client says about it, where the client has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fetching {
    /// How far along, from zero to a hundred.
    pub progress: u8,
    /// Whether it has moved since it was last looked at.
    pub moving: bool,
}

impl Fetching {
    /// Whether the client considers the transfer finished.
    #[must_use]
    pub const fn is_complete(self) -> bool {
        self.progress >= 100
    }
}

/// What an \*arr says about it, where an \*arr has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Importing {
    /// How many times importing it has failed. Zero where it has not been tried
    /// or has not failed.
    pub failures: u32,
    /// Whether the \*arr has finished with it — imported and done.
    pub imported: bool,
}

/// One thing in the pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// What it is, as both sides name it — which is what correlates them.
    pub name: String,
    /// What the client says, or nothing where no client has it.
    pub fetching: Option<Fetching>,
    /// What an \*arr says, or nothing where none is waiting for it.
    pub importing: Option<Importing>,
    /// How long it has been in this state.
    pub held_for: Duration,
    /// How many times this same item has been fetched. More than once is the
    /// signal that something is retrying an import that keeps failing.
    pub grabs: u32,
    /// What the service said was blocking it, in its own words, where it said
    /// anything. Carried verbatim: a permission denial from an import log is worth
    /// more than any interpretation of it, and where several items report the same
    /// one it is the cause rather than the items that is wrong.
    pub cause: Option<String>,
    /// The operator said to leave this one alone. Nothing is reported about it,
    /// whatever it is doing — a queue check that keeps flagging something already
    /// judged is a check that gets muted.
    pub unmanaged: bool,
}

impl Item {
    /// A plain item nothing has happened to yet, for a caller to fill in.
    #[must_use]
    pub fn named(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            fetching: None,
            importing: None,
            held_for: Duration::ZERO,
            grabs: 1,
            cause: None,
            unmanaged: false,
        }
    }

    /// Whether it has finished downloading and nothing has taken it.
    ///
    /// The failure nobody owns. An \*arr that has imported it is done — the client
    /// keeping the file to seed is not a problem, it is the arrangement working.
    #[must_use]
    pub(crate) fn is_completed_not_imported(&self) -> bool {
        let finished = self.fetching.is_some_and(Fetching::is_complete);
        finished && !self.importing.is_some_and(|importing| importing.imported)
    }

    /// Whether it is on disk with nothing waiting for it.
    #[must_use]
    pub(crate) const fn is_orphaned(&self) -> bool {
        self.fetching.is_some() && self.importing.is_none()
    }

    /// Whether nothing has been fetched for it at all.
    #[must_use]
    pub(crate) const fn is_waiting(&self) -> bool {
        self.fetching.is_none()
    }
}

#[cfg(test)]
mod tests;
