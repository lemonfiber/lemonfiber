//! The ways an item stops moving, and what to do about each.
//!
//! Categories exist because the remedies differ. "Three items stuck" is a status
//! line; an operator can do nothing with it. A dead torrent wants removing and
//! re-grabbing, a permission denial wants fixing on the volume, and a redownload
//! loop wants stopping before it eats another hundred gigabytes of a Usenet
//! allowance — and telling an operator all three are "stuck" hides the only part
//! of the message that would have helped.
//!
//! The redownload loop earns its own category for that reason. It looks like
//! ordinary activity from every angle: the client is downloading, the \*arr is
//! importing, both report success, and it happens again an hour later. Nothing
//! that watches one service can see it.

use serde::{Deserialize, Serialize};

/// Why an item is not moving.
///
/// Ordered by how much of the operator's attention each deserves, worst first, so
/// a summary that leads with the worst category needs no second ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stall {
    /// Fetched over and over: import is failing silently and being retried, which
    /// spends bandwidth and Usenet allowance indefinitely while looking normal.
    RedownloadLoop,
    /// The same item has failed to import more than once. Structural — it will not
    /// resolve itself.
    RepeatedImportFailure,
    /// Downloaded successfully and never imported. The failure nobody owns: the
    /// client considers it finished, the \*arr never picked it up, and from each
    /// service's own perspective there is nothing wrong.
    CompletedNotImported,
    /// On disk, and no \*arr knows about it.
    Orphaned,
    /// No progress at all beyond the threshold.
    StalledDownload,
    /// Monitored, never grabbed — nothing matched, or the indexers are answering
    /// with nothing.
    WaitingIndefinitely,
    /// Moving, but slowly enough to be worth knowing about. Deliberately its own
    /// category rather than a stall: something still arriving needs patience, not
    /// intervention, and reporting the two alike is how a queue check gets muted.
    Slow,
}

impl Stall {
    /// Every category, worst first.
    pub const ALL: [Self; 7] = [
        Self::RedownloadLoop,
        Self::RepeatedImportFailure,
        Self::CompletedNotImported,
        Self::Orphaned,
        Self::StalledDownload,
        Self::WaitingIndefinitely,
        Self::Slow,
    ];

    /// What the operator reads.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::RedownloadLoop => "fetched over and over",
            Self::RepeatedImportFailure => "failing to import, repeatedly",
            Self::CompletedNotImported => "downloaded but never imported",
            Self::Orphaned => "on disk, and nothing is waiting for it",
            Self::StalledDownload => "not moving",
            Self::WaitingIndefinitely => "waiting, with nothing found",
            Self::Slow => "moving slowly",
        }
    }

    /// What is usually behind it, so the operator knows where to look before they
    /// have opened anything.
    #[must_use]
    pub const fn typically(self) -> &'static str {
        match self {
            Self::RedownloadLoop => "an import that fails quietly and is retried",
            Self::RepeatedImportFailure => {
                "a permission, an unparsable name, or an archive nothing extracted"
            }
            Self::CompletedNotImported => {
                "the file landed somewhere the *arr is not looking, or cannot read"
            }
            Self::Orphaned => "added by hand, or the *arr lost track of it",
            Self::StalledDownload => "no seeders, or the article is past retention",
            Self::WaitingIndefinitely => "nothing matches, or an indexer stopped answering",
            Self::Slow => "a slow source, or something else using the connection",
        }
    }

    /// What to do about it, most likely first. Never empty.
    #[must_use]
    pub fn remedies(self) -> Vec<String> {
        let each = match self {
            Self::RedownloadLoop => [
                "stop the item before it spends more of the allowance, then fix the import",
                "check the *arr's history for the same item arriving repeatedly",
            ],
            Self::RepeatedImportFailure => [
                "read the *arr's import log for what it refused, and fix that",
                "import it by hand once, to see the refusal directly",
            ],
            Self::CompletedNotImported => [
                "check the download and library paths agree between the client and the *arr",
                "check the container can read where the file landed",
            ],
            Self::Orphaned => [
                "remove it if nothing wants it, or add the series or film that does",
                "check whether an *arr lost track of something it had grabbed",
            ],
            Self::StalledDownload => [
                "remove it and grab a different release",
                "check the indexer still has something to offer for it",
            ],
            Self::WaitingIndefinitely => [
                "check the indexers are answering — an expired key returns nothing quietly",
                "widen the quality profile if nothing available can satisfy it",
            ],
            Self::Slow => [
                "leave it — something still arriving needs patience rather than intervention",
                "check whether something else is using the connection",
            ],
        };
        each.iter().map(|remedy| (*remedy).to_owned()).collect()
    }

    /// Whether this is worth interrupting the operator about, as opposed to worth
    /// showing them where they are already looking.
    ///
    /// Slow is not: it is moving, and an alert for something that is working is
    /// how a queue check gets muted along with everything else on the channel.
    #[must_use]
    pub const fn wants_attention(self) -> bool {
        !matches!(self, Self::Slow)
    }
}

#[cfg(test)]
mod tests {
    use super::Stall;

    #[test]
    fn every_category_says_what_it_is_what_causes_it_and_what_to_do() {
        // A category that cannot answer all three is a status line wearing a name,
        // which is the thing this exists instead of.
        for stall in Stall::ALL {
            assert!(!stall.word().is_empty(), "{stall:?}");
            assert!(!stall.typically().is_empty(), "{stall:?}");
            assert!(!stall.remedies().is_empty(), "{stall:?}");
        }
    }

    #[test]
    fn no_two_categories_offer_the_same_advice() {
        // If two did, they would not be two categories — they would be one with a
        // spelling difference, and the operator would learn the distinction is
        // decorative.
        let mut first: Vec<String> = Stall::ALL
            .iter()
            .filter_map(|stall| stall.remedies().first().cloned())
            .collect();
        let count = first.len();
        first.sort_unstable();
        first.dedup();
        assert_eq!(
            first.len(),
            count,
            "two categories give the same first advice"
        );
    }

    #[test]
    fn the_worst_thing_sorts_first() {
        // Declaration order is the ranking, so a summary leading with the worst
        // needs no second ordering to keep in step with this one.
        let mut shuffled = vec![Stall::Slow, Stall::RedownloadLoop, Stall::StalledDownload];
        shuffled.sort_unstable();
        assert_eq!(
            shuffled,
            vec![Stall::RedownloadLoop, Stall::StalledDownload, Stall::Slow]
        );
    }

    #[test]
    fn something_still_arriving_is_not_worth_an_interruption() {
        // An alert about something that is working is how the whole check gets
        // muted, and the leak alert with it.
        assert!(!Stall::Slow.wants_attention());
        for stall in Stall::ALL.iter().filter(|stall| **stall != Stall::Slow) {
            assert!(stall.wants_attention(), "{stall:?}");
        }
    }
}
