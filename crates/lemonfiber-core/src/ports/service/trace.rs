//! Reading the fragments a "where is my show?" trace is assembled from — one \*arr's view
//! of an item's journey, and the media server's word on whether it is finally playable.
//! The D9 additions to the service port, kept apart from the provisioning surface.

use async_trait::async_trait;

use super::Failure;

/// One library item a trace could follow: its id, the title it was found by, and
/// whether the service is monitoring it — the entry point for "where is my show?".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundItem {
    /// The service's own id for the item, used to read its history.
    pub id: i64,
    /// The item's title, as a person would name it.
    pub title: String,
    /// Whether the service is monitoring it — an unmonitored item is one nobody asked
    /// for, the first of the stages a trace tells apart.
    pub monitored: bool,
}

/// One notable event in an item's history: what happened and when. The forward-moving
/// outcomes — a grab, an import — mark how far the item got; the rest — a failed
/// download, a removal — are the history of what has been tried, shown even though they
/// advance no stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    /// What happened.
    pub outcome: crate::trace::Outcome,
    /// When it happened, as the service reported it.
    pub at: String,
    /// The part of the item it happened to — an episode — where the service files its
    /// history that way. A film's history names no part.
    pub part: Option<i64>,
}

/// One record the download queue holds for an item: the stage it is at, and whether it is
/// stuck — queued but not progressing, the C7 signal.
///
/// A series holds one record per episode, so this is per-record rather than collapsed to
/// the item: an episode downloading now and one grabbed and lost look identical once
/// flattened, and telling those apart is the whole value of a trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueItem {
    /// The part of the item this record is for — an episode — where the service names
    /// one. A film's queue record names no part; the record is for the whole item.
    pub part: Option<i64>,
    /// The furthest stage the queue shows this record at.
    pub stage: crate::trace::Stage,
    /// Whether it is stuck — a warning or error tracked-download status.
    pub stuck: bool,
}

/// One part of a library item as the service records it now — an episode of a series.
///
/// Only what the listing genuinely carries: the file and the monitored flag. Whether a
/// release was grabbed is deliberately not here — the television service defines such a
/// field on its episode type but never populates it on this listing, so reading it would
/// have reported every grabbed episode as one the indexers never found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemPart {
    /// The service's own id for the part, which its queue records name.
    pub id: i64,
    /// The season it belongs to. Season zero is where a service files specials.
    pub season: u32,
    /// Its number within that season.
    pub number: u32,
    /// Its title, as a person would name it.
    pub title: String,
    /// Whether the service is monitoring it — whether anyone asked for this part.
    pub monitored: bool,
    /// Whether its file is on disk.
    pub has_file: bool,
}

/// A stuck item the queue is holding — one queue health reports so it can be traced. Its
/// title is the human term a trace searches by, so "3 items stuck" leads straight to the
/// per-item explanation rather than to a count the operator must go and investigate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StuckItem {
    /// The item's title, as a person would name it — the term its trace is searched by.
    pub title: String,
    /// The stage its download is stuck at.
    pub stage: crate::trace::Stage,
}

/// Reading one \*arr's fragment of an item's journey — the service that monitors the
/// item and records its history is the spine a trace is built along; the other services
/// fill in around it.
#[async_trait]
pub trait Pipeline: Send + Sync {
    /// Every item the service's library holds — the one read a title is found through,
    /// whether by a human term or by the id another service knows the item as.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn library(&self, kind: crate::recyclarr::Kind) -> Result<Vec<FoundItem>, Failure>;

    /// Find library items whose title matches a human term — a show name or a film
    /// title, never an internal id.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn find_items(
        &self,
        kind: crate::recyclarr::Kind,
        term: &str,
    ) -> Result<Vec<FoundItem>, Failure>;

    /// Read the notable history of one item, newest first — the grabs, failed downloads,
    /// imports and removals that show both how far it got and what has been tried.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn item_history(
        &self,
        kind: crate::recyclarr::Kind,
        id: i64,
    ) -> Result<Vec<TraceEvent>, Failure>;

    /// What the download queue holds for an item now, one entry per record — empty where
    /// it holds nothing, so the trace reads on from history alone. A series in flight
    /// yields one entry per episode, each naming the part it is for.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn item_queue(
        &self,
        kind: crate::recyclarr::Kind,
        id: i64,
    ) -> Result<Vec<QueueItem>, Failure>;

    /// The parts of one item — the episodes of a series, narrowed to one season where
    /// `season` names one. A film has no parts and yields none: the item is the whole,
    /// and a trace of it already says all there is to say.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn item_parts(
        &self,
        kind: crate::recyclarr::Kind,
        id: i64,
        season: Option<u32>,
    ) -> Result<Vec<ItemPart>, Failure>;

    /// The items the queue is holding that are stuck — each named by its title so it links
    /// to its own trace. The bridge from "queue health says N are stuck" to the per-item
    /// explanation each one has.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn stuck_items(&self, kind: crate::recyclarr::Kind) -> Result<Vec<StuckItem>, Failure>;
}

/// Reading a media server's library to answer the last question a trace has left — is
/// the item finally visible and playable? Jellyfin, the one service lemonfiber holds the
/// household's own admin credential for, so it can ask on the household's behalf.
///
/// A title is the only join across to the media server — it shares no id with the \*arr
/// that traced the item this far — so a match here is a fuzzy one, and the trace marks
/// the availability it yields uncertain rather than claim a title-guess as fact.
#[async_trait]
pub trait Library: Send + Sync {
    /// Whether an item whose title matches `term` is present in the library, for the
    /// media type `kind` names — a series for television, a movie for film.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the media server is unreachable or refuses — which a
    /// trace reads as "cannot tell", never as "not there".
    async fn has_item(&self, kind: crate::recyclarr::Kind, term: &str) -> Result<bool, Failure>;

    /// Tell the media server to look at what is on disk now.
    ///
    /// A library scans on its own schedule, so content that has just been imported is
    /// genuinely there and genuinely invisible for as long as an hour. Left alone that is
    /// the walkthrough's worst possible ending — everything worked and nothing is there —
    /// so it is asked for rather than waited on.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the media server is unreachable or refuses.
    async fn rescan(&self) -> Result<(), Failure>;
}
