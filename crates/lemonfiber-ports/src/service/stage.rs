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
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
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
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "TraceConfidence")]
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

    /// The stage one part of an item rests at on the service's own current record: its
    /// file and its monitored flag. This is where a part starts from, before what was
    /// tried for it is folded in.
    ///
    /// A part already on disk is here whether or not anyone is still monitoring it: the
    /// file is the fact, and calling it "nobody asked for it" would be a worse answer
    /// than the truth. Only a part that is both unmonitored and absent is one nobody
    /// asked for.
    #[must_use]
    pub const fn of_part(monitored: bool, has_file: bool) -> Self {
        if has_file {
            Self::Imported
        } else if monitored {
            Self::Monitored
        } else {
            Self::NotMonitored
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
            Self::Monitored => Some("monitored, but nothing has been grabbed for it yet"),
            Self::Found => Some(
                "found, but nothing met the quality preset — releases are out there and the \
                 quality in force rejects every one, so easing the preset is what gets this",
            ),
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "TraceOutcome")]
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
mod tests;
