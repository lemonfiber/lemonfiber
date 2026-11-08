//! How the queue heals, as `lemonfiber queue` reads it.

use serde::{Deserialize, Serialize};

/// Whether healing acts, the strikes and grace it acts after, every unhealthy item of the
/// latest pass, what could not be reached, and the newest runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct QueueHealth {
    /// Whether healing acts, or only classifies.
    pub healing: HealingSwitch,
    /// How many consecutive passes flag an item for one reason before it is acted on.
    pub strikes_needed: u32,
    /// How long a newly added download is left alone, in seconds.
    pub grace_s: u64,
    /// Every unhealthy item the latest pass found; a healthy one is left out.
    #[serde(default)]
    pub items: Vec<Wedged>,
    /// Every curator or download client the latest pass could not reach or verify.
    #[serde(default)]
    pub unreachable: Vec<Unreached>,
    /// Where a download client files a curator's downloads outside the curator's folder,
    /// which nothing is acted on past.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mismatch: Option<String>,
    /// Whether the list may be short, because something could not be read.
    pub incomplete: bool,
    /// The newest runs first, each a pass that could act, rehearsed or not.
    #[serde(default)]
    pub runs: Vec<HealRun>,
    /// When the latest pass classified the queue, in seconds since the Unix epoch, where
    /// one ever has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classified_at: Option<u64>,
    /// Whether this answer is a run that only said what it would do.
    pub rehearsed: bool,
}

/// Whether healing acts.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum HealingSwitch {
    /// It acts on what has struck out.
    On,
    /// It only classifies.
    #[default]
    Off,
}

/// Why a download is unhealthy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Wedge {
    /// Connected, with nothing moving and no source to move it.
    Stalled,
    /// Moving, but so slowly its estimate runs past what is allowed.
    Slow,
    /// The client or the curator gave up on it.
    Failed,
    /// A torrent that has not resolved what it is.
    StuckOnMetadata,
    /// In a client under a curator's category, and in no curator's queue.
    Orphaned,
}

/// One unhealthy item, as the latest pass found it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Wedged {
    /// The title it is for, where a curator holds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The curator it is queued with, or the client holding it where none is.
    pub service: String,
    /// The release, by its name in the client.
    pub release: String,
    /// Why it is unhealthy.
    pub class: Wedge,
    /// Why, in words.
    pub why: String,
    /// How many consecutive passes have flagged it for this reason.
    pub strikes: u32,
    /// When it was added, in seconds since the Unix epoch, where anything says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_at: Option<u64>,
    /// Until when it is left alone as newly added, where it still is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_grace_until: Option<u64>,
    /// What the next run would do to it, where healing is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Vec<Act>>,
}

/// A curator or download client a pass could not reach or verify.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Unreached {
    /// The service, by its id.
    pub service: String,
    /// Why, in words.
    pub why: String,
}

/// One of the three acts a run takes on an item that struck out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Act {
    /// Drop the download from its client.
    Remove,
    /// Keep its release from being grabbed again.
    Blocklist,
    /// Search for the title again.
    Search,
}

/// What became of one act.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ActOutcome {
    /// It was done.
    Done,
    /// It would have been done; the run was a rehearsal.
    Would,
    /// It was not done.
    Refused,
    /// Something weaker was done in its place.
    FellBack,
}

/// One act as a run took it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ActTaken {
    /// The act.
    pub act: Act,
    /// What became of it.
    pub outcome: ActOutcome,
    /// What was done in its place, where something weaker was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instead: Option<Act>,
    /// Why, where it was refused or fell back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// One item a run acted on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Healed {
    /// The title it was for, where a curator held it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The curator it was queued with, or the client holding it where none was.
    pub service: String,
    /// The release, by its name in the client.
    pub release: String,
    /// Why it was acted on.
    pub why: String,
    /// Each act, in the order they were taken.
    pub acts: Vec<ActTaken>,
}

/// One run: a pass that could act.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HealRun {
    /// When, in seconds since the Unix epoch.
    pub at: u64,
    /// Whether it was a rehearsal, which acted on nothing.
    pub rehearsed: bool,
    /// Every item it acted on, or would have.
    #[serde(default)]
    pub items: Vec<Healed>,
}
