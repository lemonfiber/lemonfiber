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
            unmanaged: false,
        }
    }

    /// Whether it has finished downloading and nothing has taken it.
    ///
    /// The failure nobody owns. An \*arr that has imported it is done — the client
    /// keeping the file to seed is not a problem, it is the arrangement working.
    #[must_use]
    pub fn is_completed_not_imported(&self) -> bool {
        let finished = self.fetching.is_some_and(Fetching::is_complete);
        finished && !self.importing.is_some_and(|importing| importing.imported)
    }

    /// Whether it is on disk with nothing waiting for it.
    #[must_use]
    pub const fn is_orphaned(&self) -> bool {
        self.fetching.is_some() && self.importing.is_none()
    }

    /// Whether nothing has been fetched for it at all.
    #[must_use]
    pub const fn is_waiting(&self) -> bool {
        self.fetching.is_none()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Fetching, Importing, Item};

    /// An item the client is part-way through.
    fn downloading(progress: u8) -> Item {
        Item {
            fetching: Some(Fetching {
                progress,
                moving: true,
            }),
            importing: Some(Importing {
                failures: 0,
                imported: false,
            }),
            ..Item::named("Some.Release")
        }
    }

    #[test]
    fn a_finished_download_nothing_took_is_the_failure_nobody_owns() {
        // To the client it is a completed download; to the *arr it is nothing at
        // all. Neither reports a problem, because neither has one.
        assert!(downloading(100).is_completed_not_imported());
        assert!(!downloading(94).is_completed_not_imported());
    }

    #[test]
    fn a_file_kept_for_seeding_after_it_was_imported_is_the_arrangement_working() {
        // The one case that looks identical from the client's side and is fine.
        let seeding = Item {
            importing: Some(Importing {
                failures: 0,
                imported: true,
            }),
            ..downloading(100)
        };
        assert!(!seeding.is_completed_not_imported());
    }

    #[test]
    fn something_on_disk_that_nothing_is_waiting_for_is_orphaned() {
        let orphan = Item {
            importing: None,
            ..downloading(100)
        };
        assert!(orphan.is_orphaned());
        assert!(!downloading(100).is_orphaned());
    }

    #[test]
    fn something_nothing_has_fetched_is_waiting_rather_than_stalled() {
        // A film that is not out yet has no download to be stalled.
        let wanted = Item {
            fetching: None,
            ..downloading(0)
        };
        assert!(wanted.is_waiting());
        assert!(!downloading(0).is_waiting());
    }

    #[test]
    fn a_fresh_item_has_been_grabbed_once_and_is_managed() {
        // The defaults matter: a caller filling in one field must not accidentally
        // declare something unmanaged or never grabbed.
        let item = Item::named("Some.Release");
        assert_eq!(item.grabs, 1);
        assert!(!item.unmanaged);
        assert_eq!(item.held_for, Duration::ZERO);
    }
}
