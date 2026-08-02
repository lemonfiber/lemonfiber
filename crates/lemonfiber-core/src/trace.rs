//! Where an item is in the pipeline, as one ordered set of stages.
//!
//! "Where is my show?" is the question a household actually asks, and answering it
//! means following one item across every service that touched it — monitored in the
//! \*arr, found by the indexer, grabbed and downloaded by the client, imported to disk,
//! visible in the library. Each service holds one fragment; none of them link.
//!
//! This is the pure spine of that answer: the stages an item passes through, in order,
//! and — the confusing part — what it means to have stopped at each one. Content that
//! never appears looks identical from outside whatever the cause, so the value is in
//! telling the causes apart: not monitored, monitored-but-never-found, found-but-never-
//! grabbed, and so on. Reading the fragments and joining them is a separate concern;
//! nothing here reaches a service.

use serde::{Deserialize, Serialize};

/// A stage in an item's journey, ordered from "nobody asked for it" to "playable". The
/// declaration order is the pipeline order, so one stage compares less than a later one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Nobody has asked for it — no \*arr is monitoring it.
    #[default]
    NotMonitored,
    /// Monitored, waiting to be searched for.
    Monitored,
    /// A search is running.
    Searching,
    /// Releases were found by an indexer.
    Found,
    /// A release was sent to the download client.
    Grabbed,
    /// The download is in progress.
    Downloading,
    /// The download finished.
    Downloaded,
    /// The \*arr is importing it to the library.
    Importing,
    /// It was imported to the library on disk.
    Imported,
    /// It is visible and playable in the media server.
    Available,
}

/// How sure the correlation behind a trace is — a release renamed between services can
/// only be matched fuzzily, and a guess presented as fact is worse than a marked one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confidence {
    /// Joined on identifiers the services agree on.
    #[default]
    Certain,
    /// Joined by fuzzy matching; the trace may not be the item asked for.
    Uncertain,
}

impl Stage {
    /// Every stage, in pipeline order.
    pub const ALL: [Self; 10] = [
        Self::NotMonitored,
        Self::Monitored,
        Self::Searching,
        Self::Found,
        Self::Grabbed,
        Self::Downloading,
        Self::Downloaded,
        Self::Importing,
        Self::Imported,
        Self::Available,
    ];

    /// The stage's stored name — the plain term a trace reports it under.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotMonitored => "not-monitored",
            Self::Monitored => "monitored",
            Self::Searching => "searching",
            Self::Found => "found",
            Self::Grabbed => "grabbed",
            Self::Downloading => "downloading",
            Self::Downloaded => "downloaded",
            Self::Importing => "importing",
            Self::Imported => "imported",
            Self::Available => "available",
        }
    }

    /// Whether this stage is work in progress rather than a resting point — a search
    /// running, a download under way, an import happening. An item resting at one of
    /// these is doing fine; an item resting at a non-progress stage below `Available`
    /// has stopped, and [`Stage::stall`] says why.
    #[must_use]
    pub const fn in_progress(self) -> bool {
        matches!(self, Self::Searching | Self::Downloading | Self::Importing)
    }

    /// The furthest stage an item reached, from whether it is monitored and the stages
    /// its history records. Unmonitored is the floor — nobody asked for it; otherwise it
    /// is the latest stage seen, or merely `Monitored` where nothing has happened yet.
    #[must_use]
    pub fn furthest(monitored: bool, reached: &[Self]) -> Self {
        if !monitored {
            return Self::NotMonitored;
        }
        reached.iter().copied().max().unwrap_or(Self::Monitored)
    }

    /// The stage a Servarr history event denotes reaching, or `None` for an event that
    /// does not advance the pipeline (a failure, a rename, a deletion) — those are read
    /// for their own meaning elsewhere, not as progress.
    ///
    /// Named as the \*arr history API serialises them: `grabbed`, and the several
    /// folder-import events the television and film services each use.
    #[must_use]
    pub fn of_event(event_type: &str) -> Option<Self> {
        match event_type {
            "grabbed" => Some(Self::Grabbed),
            "downloadFolderImported" | "seriesFolderImported" | "movieFolderImported" => {
                Some(Self::Imported)
            }
            _ => None,
        }
    }

    /// Why an item that got no further than this stage has stopped, in plain language —
    /// or `None` where stopping here is not a fault: the terminal `Available`, or a
    /// stage that is work in progress rather than a resting point.
    ///
    /// This is the heart of the trace: content that never appears looks the same from
    /// outside whatever the cause, and each cause has a different remedy. A concrete
    /// reason a service reported overrides this generic one where there is one; this is
    /// what to say when all that is known is how far the item got.
    #[must_use]
    pub const fn stall(self) -> Option<&'static str> {
        match self {
            Self::NotMonitored => Some("nobody has asked for it — no service is monitoring it"),
            Self::Monitored => {
                Some("monitored, but no search has found it yet — the indexers returned nothing")
            }
            Self::Found => {
                Some("found, but nothing met the quality preset, so nothing was grabbed")
            }
            Self::Grabbed => Some("grabbed, but the download client never took it"),
            Self::Downloaded => Some("downloaded, but it was never imported to the library"),
            Self::Imported => {
                Some("imported, but not yet visible — the library has not been scanned")
            }
            Self::Searching | Self::Downloading | Self::Importing | Self::Available => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Confidence, Stage};

    #[test]
    fn the_stages_are_declared_in_pipeline_order() {
        // A later stage compares greater, so "the furthest reached" is a max — and the
        // ALL array is that same order for a surface to walk.
        let mut sorted = Stage::ALL;
        sorted.sort_unstable();
        assert_eq!(sorted, Stage::ALL);
        assert!(Stage::NotMonitored < Stage::Available);
        assert!(Stage::Found < Stage::Grabbed);
    }

    #[test]
    fn every_stage_has_a_plain_label() {
        for stage in Stage::ALL {
            let label = stage.label();
            assert!(!label.is_empty());
            // Plain words a household reads, never an internal identifier.
            assert!(!label.contains('_'));
            assert!(label.chars().all(|c| c.is_ascii_lowercase() || c == '-'));
        }
    }

    #[test]
    fn in_progress_stages_are_the_transient_ones() {
        assert!(Stage::Searching.in_progress());
        assert!(Stage::Downloading.in_progress());
        assert!(Stage::Importing.in_progress());
        assert!(!Stage::Monitored.in_progress());
        assert!(!Stage::Available.in_progress());
    }

    #[test]
    fn a_resting_pre_terminal_stage_says_why_it_stopped() {
        // The R5 distinctions: each place an item can silently rest carries its own
        // reason, so "nothing happened" is never left ambiguous.
        for stage in [
            Stage::NotMonitored,
            Stage::Monitored,
            Stage::Found,
            Stage::Grabbed,
            Stage::Downloaded,
            Stage::Imported,
        ] {
            assert!(stage.stall().is_some(), "{} has no reason", stage.label());
        }
    }

    #[test]
    fn success_and_work_in_progress_are_not_stalls() {
        assert_eq!(Stage::Available.stall(), None);
        assert_eq!(Stage::Searching.stall(), None);
        assert_eq!(Stage::Downloading.stall(), None);
        assert_eq!(Stage::Importing.stall(), None);
    }

    #[test]
    fn the_never_found_and_never_grabbed_reasons_are_distinct() {
        // The two most-confused cases must not read the same: "indexers returned
        // nothing" is a different problem from "nothing met the preset".
        assert_ne!(Stage::Monitored.stall(), Stage::Found.stall());
    }

    #[test]
    fn a_stage_serialises_under_its_label() {
        let json = serde_json::to_string(&Stage::Downloaded).unwrap_or_default();
        assert_eq!(json, r#""downloaded""#);
        let back: Option<Stage> = serde_json::from_str(&json).ok();
        assert_eq!(back, Some(Stage::Downloaded));
    }

    #[test]
    fn the_furthest_stage_is_the_latest_reached() {
        assert_eq!(Stage::furthest(false, &[]), Stage::NotMonitored);
        // Unmonitored floors even if history somehow shows more.
        assert_eq!(
            Stage::furthest(false, &[Stage::Grabbed]),
            Stage::NotMonitored
        );
        assert_eq!(Stage::furthest(true, &[]), Stage::Monitored);
        assert_eq!(
            Stage::furthest(true, &[Stage::Grabbed, Stage::Imported]),
            Stage::Imported
        );
        // Order of the events does not matter — the furthest is a max.
        assert_eq!(
            Stage::furthest(true, &[Stage::Imported, Stage::Grabbed]),
            Stage::Imported
        );
    }

    #[test]
    fn history_events_map_to_the_stage_they_reach() {
        assert_eq!(Stage::of_event("grabbed"), Some(Stage::Grabbed));
        assert_eq!(
            Stage::of_event("downloadFolderImported"),
            Some(Stage::Imported)
        );
        assert_eq!(
            Stage::of_event("movieFolderImported"),
            Some(Stage::Imported)
        );
        assert_eq!(
            Stage::of_event("seriesFolderImported"),
            Some(Stage::Imported)
        );
    }

    #[test]
    fn a_non_advancing_event_maps_to_no_stage() {
        // A failure, rename, or deletion is not progress — it is read for its own
        // meaning, not as a stage reached.
        assert_eq!(Stage::of_event("downloadFailed"), None);
        assert_eq!(Stage::of_event("episodeFileDeleted"), None);
        assert_eq!(Stage::of_event(""), None);
    }

    #[test]
    fn confidence_serialises_under_its_label() {
        assert_eq!(
            serde_json::to_string(&Confidence::Uncertain).unwrap_or_default(),
            r#""uncertain""#
        );
    }
}
