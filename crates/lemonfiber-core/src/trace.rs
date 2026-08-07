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

/// How many of an item's most recent history events a trace reads — the bounded horizon
/// on retained detail. Stating it keeps "nothing earlier" honest: an event older than this
/// window is simply not read, which is not proof that nothing happened before it.
pub const HISTORY_HORIZON: usize = 100;

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

/// What the media server says about an item being in the library — the final stage, the
/// one no \*arr can see. Read as a three-way answer because "not in the library" is only
/// meaningful when the media server actually answered: where it could not be reached the
/// presence is simply unknown, and a trace never infers an availability it cannot confirm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// The media server has it — visible and playable, the item is available.
    Present,
    /// The media server answered and does not have it — genuinely not yet visible, so an
    /// item imported to disk is now provably still waiting for the library to be scanned.
    Absent,
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

    /// The stage one part of an item rests at, from what the service records about it
    /// now rather than from its history. A file on disk, a release already grabbed and a
    /// monitored flag are current facts, so a part's stage does not depend on how far
    /// back the bounded history horizon reaches — which matters most for exactly the long
    /// series whose early events fall outside it.
    ///
    /// A part already on disk is here whether or not anyone is still monitoring it: the
    /// file is the fact, and calling it "nobody asked for it" would be a worse answer
    /// than the truth. Only a part that is both unmonitored and absent is one nobody
    /// asked for.
    #[must_use]
    pub const fn of_part(monitored: bool, has_file: bool, grabbed: bool) -> Self {
        if has_file {
            Self::Imported
        } else if !monitored {
            Self::NotMonitored
        } else if grabbed {
            Self::Grabbed
        } else {
            Self::Monitored
        }
    }

    /// The stage a download client's tracked state denotes, or `None` for a state that
    /// is not progress on the pipeline (a failure, an ignore). Named as the \*arr queue
    /// serialises them.
    #[must_use]
    pub fn of_queue_state(state: &str) -> Option<Self> {
        match state {
            "downloading" => Some(Self::Downloading),
            "importPending" => Some(Self::Downloaded),
            "importing" => Some(Self::Importing),
            "imported" => Some(Self::Imported),
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

/// One part of a traced item — an episode of a series. A film has no parts: the item is
/// the whole, and a trace of it says all there is to say. A series does not, which is the
/// gap this closes: "the show is imported" is true the moment one episode lands, and reads
/// as done while nine are still missing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Part {
    /// Which season it belongs to.
    pub season: u32,
    /// Its number within that season.
    pub number: u32,
    /// Its title, as a person would name it.
    pub title: String,
    /// How far this one part got, on the same scale as the item as a whole.
    pub stage: Stage,
}

impl Part {
    /// Whether this part is here — imported to the library on disk or beyond.
    #[must_use]
    pub fn here(&self) -> bool {
        self.stage >= Stage::Imported
    }

    /// Whether nobody asked for this part — unmonitored and not already on disk. Kept
    /// apart from the parts that are merely missing, because the two need opposite things
    /// from an operator: one is a fault to chase, the other is a choice already made.
    #[must_use]
    pub fn unasked(&self) -> bool {
        self.stage == Stage::NotMonitored
    }
}

/// How much of one season is actually here, and what is outstanding — the season-level
/// answer, which for a series is the one an operator can act on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SeasonCoverage {
    /// The season number. Season zero is where a service files specials.
    pub season: u32,
    /// How many of the wanted parts are here.
    pub have: usize,
    /// How many parts were asked for, or are already here — the denominator. Parts
    /// nobody asked for are counted separately rather than inflating this, so a season
    /// with every wanted episode present reads as complete even where specials are not.
    pub wanted: usize,
    /// How many parts nobody asked for — unmonitored and not on disk.
    pub unmonitored: usize,
    /// The wanted parts that are not here yet, each carrying the stage it rests at, so
    /// one that stalled is told apart from one still downloading.
    pub outstanding: Vec<Part>,
}

/// How much of a traced series is here, season by season — the aggregate that turns a
/// single furthest stage into an answer about the whole. The counts are of parts someone
/// asked for; what nobody asked for is reported beside them, never folded in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Coverage {
    /// Each season, in order.
    pub seasons: Vec<SeasonCoverage>,
    /// How many wanted parts are here, across every season.
    pub have: usize,
    /// How many parts were asked for, across every season.
    pub wanted: usize,
    /// How many parts nobody asked for, across every season.
    pub unmonitored: usize,
}

impl Coverage {
    /// Aggregate the parts of an item into the per-season summary. Seasons come out in
    /// number order whatever order the service listed the parts in, so the reading is
    /// stable; a part nobody asked for counts toward `unmonitored` and toward nothing
    /// else.
    #[must_use]
    pub fn of(parts: Vec<Part>) -> Self {
        let mut by_season: std::collections::BTreeMap<u32, Vec<Part>> =
            std::collections::BTreeMap::new();
        for part in parts {
            by_season.entry(part.season).or_default().push(part);
        }

        let mut coverage = Self::default();
        for (season, mut parts) in by_season {
            parts.sort_by_key(|part| part.number);
            let unmonitored = parts.iter().filter(|part| part.unasked()).count();
            let have = parts.iter().filter(|part| part.here()).count();
            let wanted = parts.len() - unmonitored;
            coverage.seasons.push(SeasonCoverage {
                season,
                have,
                wanted,
                unmonitored,
                outstanding: parts
                    .into_iter()
                    .filter(|part| !part.here() && !part.unasked())
                    .collect(),
            });
            coverage.have += have;
            coverage.wanted += wanted;
            coverage.unmonitored += unmonitored;
        }
        coverage
    }

    /// Whether every wanted part is here — the plain "is it complete?" a household asks.
    /// A series nothing is monitored on is not complete; there is nothing to be complete.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.wanted > 0 && self.have == self.wanted
    }
}

impl SeasonCoverage {
    /// Whether every wanted part of this season is here.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.wanted > 0 && self.have == self.wanted
    }
}

/// A notable thing that happened to an item, as an \*arr's history records it. Where the
/// furthest stage answers "how far did it get?", the sequence of outcomes answers "what
/// has been tried?" — a release grabbed more than once, a download that failed and was
/// tried again, a file imported and later removed. Repeated failed grabs are a pattern
/// worth seeing, not something a single furthest-stage reading can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// A release was sent to the download client.
    Grabbed,
    /// The download client failed the release it was handed.
    DownloadFailed,
    /// The item was imported to the library on disk.
    Imported,
    /// The item's file was removed.
    Removed,
}

impl Outcome {
    /// The outcome an \*arr history event denotes, or `None` for an event that is not a
    /// notable step in the item's history. Named as the television and film services each
    /// serialise their history event types.
    #[must_use]
    pub fn of_event(event_type: &str) -> Option<Self> {
        match event_type {
            "grabbed" => Some(Self::Grabbed),
            "downloadFailed" => Some(Self::DownloadFailed),
            "downloadFolderImported" | "seriesFolderImported" | "movieFolderImported" => {
                Some(Self::Imported)
            }
            "episodeFileDeleted" | "movieFileDeleted" => Some(Self::Removed),
            _ => None,
        }
    }

    /// The pipeline stage this outcome carries the item to, or `None` where it is not
    /// forward progress — a failed download or a removal is history to show, not a stage
    /// the item reached, so it never advances how far the item got.
    #[must_use]
    pub const fn stage(self) -> Option<Stage> {
        match self {
            Self::Grabbed => Some(Stage::Grabbed),
            Self::Imported => Some(Stage::Imported),
            Self::DownloadFailed | Self::Removed => None,
        }
    }

    /// The plain-language phrase a trace's history names this outcome by.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            Self::Grabbed => "grabbed",
            Self::DownloadFailed => "download failed",
            Self::Imported => "imported",
            Self::Removed => "removed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Confidence, Coverage, Outcome, Part, Stage};

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
    fn history_events_map_to_the_outcome_they_denote() {
        assert_eq!(Outcome::of_event("grabbed"), Some(Outcome::Grabbed));
        assert_eq!(
            Outcome::of_event("downloadFailed"),
            Some(Outcome::DownloadFailed)
        );
        for imported in [
            "downloadFolderImported",
            "movieFolderImported",
            "seriesFolderImported",
        ] {
            assert_eq!(Outcome::of_event(imported), Some(Outcome::Imported));
        }
        assert_eq!(
            Outcome::of_event("episodeFileDeleted"),
            Some(Outcome::Removed)
        );
        assert_eq!(
            Outcome::of_event("movieFileDeleted"),
            Some(Outcome::Removed)
        );
    }

    #[test]
    fn only_a_forward_outcome_advances_the_stage() {
        // A grab and an import move the item along; a failure or a removal is history to
        // show, not a stage reached.
        assert_eq!(Outcome::Grabbed.stage(), Some(Stage::Grabbed));
        assert_eq!(Outcome::Imported.stage(), Some(Stage::Imported));
        assert_eq!(Outcome::DownloadFailed.stage(), None);
        assert_eq!(Outcome::Removed.stage(), None);
    }

    #[test]
    fn every_outcome_has_a_plain_phrase() {
        for outcome in [
            Outcome::Grabbed,
            Outcome::DownloadFailed,
            Outcome::Imported,
            Outcome::Removed,
        ] {
            let phrase = outcome.phrase();
            assert!(!phrase.is_empty());
            assert!(phrase.chars().all(|c| c.is_ascii_lowercase() || c == ' '));
        }
    }

    #[test]
    fn queue_states_map_to_the_stage_they_are_at() {
        assert_eq!(
            Stage::of_queue_state("downloading"),
            Some(Stage::Downloading)
        );
        assert_eq!(
            Stage::of_queue_state("importPending"),
            Some(Stage::Downloaded)
        );
        assert_eq!(Stage::of_queue_state("importing"), Some(Stage::Importing));
        assert_eq!(Stage::of_queue_state("imported"), Some(Stage::Imported));
        assert_eq!(Stage::of_queue_state("failed"), None);
        assert_eq!(Stage::of_queue_state(""), None);
    }

    #[test]
    fn an_event_that_is_not_notable_maps_to_no_outcome() {
        // A rename, a grab-from-interactive-search test, or an unknown event is not one
        // of the history moments a trace shows.
        assert_eq!(Outcome::of_event("episodeFileRenamed"), None);
        assert_eq!(Outcome::of_event(""), None);
    }

    /// A part at a given season, number and stage — the shape the aggregation groups.
    fn part(season: u32, number: u32, stage: Stage) -> Part {
        Part {
            season,
            number,
            title: format!("S{season:02}E{number:02}"),
            stage,
        }
    }

    #[test]
    fn a_part_on_disk_is_here_whoever_stopped_monitoring_it() {
        // The file is the fact. An episode grabbed and then unmonitored is still here,
        // and calling it "nobody asked for it" would be the worse answer.
        assert_eq!(Stage::of_part(false, true, false), Stage::Imported);
        assert_eq!(Stage::of_part(true, true, false), Stage::Imported);
    }

    #[test]
    fn an_unmonitored_absent_part_is_one_nobody_asked_for() {
        assert_eq!(Stage::of_part(false, false, false), Stage::NotMonitored);
        // Monitoring stopped mid-flight still means nobody is asking for it now.
        assert_eq!(Stage::of_part(false, false, true), Stage::NotMonitored);
    }

    #[test]
    fn a_wanted_part_reads_from_whether_it_was_grabbed() {
        assert_eq!(Stage::of_part(true, false, true), Stage::Grabbed);
        assert_eq!(Stage::of_part(true, false, false), Stage::Monitored);
    }

    #[test]
    fn coverage_counts_what_was_asked_for_and_reports_the_rest_apart() {
        // A season of three wanted episodes, one here — plus a special nobody asked for,
        // which must not drag the denominator to four and read as a fault.
        let coverage = Coverage::of(vec![
            part(1, 2, Stage::Monitored),
            part(1, 1, Stage::Imported),
            part(1, 3, Stage::Downloading),
            part(0, 1, Stage::NotMonitored),
        ]);
        assert_eq!(coverage.have, 1);
        assert_eq!(coverage.wanted, 3);
        assert_eq!(coverage.unmonitored, 1);
        // Seasons come out in number order, specials first as season zero.
        let numbers: Vec<u32> = coverage.seasons.iter().map(|s| s.season).collect();
        assert_eq!(numbers, vec![0, 1]);
    }

    #[test]
    fn outstanding_parts_are_the_wanted_ones_not_here_yet_in_order() {
        let coverage = Coverage::of(vec![
            part(2, 3, Stage::Monitored),
            part(2, 1, Stage::Imported),
            part(2, 2, Stage::Downloading),
            part(2, 4, Stage::NotMonitored),
        ]);
        let season = coverage.seasons.first().cloned().unwrap_or_default();
        // Sorted by number whatever order the service listed them in, so the reading is
        // stable; the one nobody asked for is not outstanding, and the one already here
        // is not either.
        let outstanding: Vec<u32> = season.outstanding.iter().map(|p| p.number).collect();
        assert_eq!(outstanding, vec![2, 3]);
        // Each carries its own stage, so a download in flight is told apart from a stall.
        let stages: Vec<Stage> = season.outstanding.iter().map(|part| part.stage).collect();
        assert_eq!(stages, vec![Stage::Downloading, Stage::Monitored]);
    }

    #[test]
    fn a_season_is_complete_when_every_wanted_part_is_here() {
        let coverage = Coverage::of(vec![
            part(1, 1, Stage::Imported),
            part(1, 2, Stage::Imported),
        ]);
        assert!(coverage.complete());
        let only = coverage.seasons.first().cloned().unwrap_or_default();
        assert!(only.complete());
        assert_eq!(only.wanted, 2);
    }

    #[test]
    fn a_series_nobody_monitors_is_not_complete() {
        // Nothing wanted is not the same as everything here — with no denominator there
        // is nothing to be complete, and reporting it complete would be a lie of shape.
        let coverage = Coverage::of(vec![part(1, 1, Stage::NotMonitored)]);
        assert!(!coverage.complete());
        let only = coverage.seasons.first().cloned().unwrap_or_default();
        assert!(!only.complete());
        assert_eq!(only.unmonitored, 1);
        // And an item with no parts at all — a film — is not a complete series either.
        assert!(!Coverage::of(Vec::new()).complete());
    }

    #[test]
    fn a_part_reports_whether_it_is_here_and_whether_it_was_asked_for() {
        assert!(part(1, 1, Stage::Imported).here());
        assert!(part(1, 1, Stage::Available).here());
        assert!(!part(1, 1, Stage::Downloading).here());
        assert!(part(1, 1, Stage::NotMonitored).unasked());
        assert!(!part(1, 1, Stage::Monitored).unasked());
    }

    #[test]
    fn confidence_serialises_under_its_label() {
        assert_eq!(
            serde_json::to_string(&Confidence::Uncertain).unwrap_or_default(),
            r#""uncertain""#
        );
    }
}
