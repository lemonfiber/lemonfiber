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

/// One stage-advancing event in an item's history: the stage it reached and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    /// The stage the event denotes reaching.
    pub stage: crate::trace::Stage,
    /// When it happened, as the service reported it.
    pub at: String,
}

/// What the download queue currently holds for an item: the stage it is at, and whether
/// it is stuck — queued but not progressing, the C7 signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueItem {
    /// The furthest stage the queue shows the item at.
    pub stage: crate::trace::Stage,
    /// Whether the item is stuck — a warning or error tracked-download status.
    pub stuck: bool,
}

/// Reading one \*arr's fragment of an item's journey — the service that monitors the
/// item and records its history is the spine a trace is built along; the other services
/// fill in around it.
#[async_trait]
pub trait Pipeline: Send + Sync {
    /// Find monitored library items whose title matches a human term — a show name or a
    /// film title, never an internal id.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn find_items(
        &self,
        kind: crate::recyclarr::Kind,
        term: &str,
    ) -> Result<Vec<FoundItem>, Failure>;

    /// Read the stage-advancing history of one item, newest first — the grabs and
    /// imports that mark how far it got.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn item_history(
        &self,
        kind: crate::recyclarr::Kind,
        id: i64,
    ) -> Result<Vec<TraceEvent>, Failure>;

    /// What the download queue holds for an item now, or `None` where it holds nothing —
    /// the item is not downloading, so the trace reads on from history alone.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn item_queue(
        &self,
        kind: crate::recyclarr::Kind,
        id: i64,
    ) -> Result<Option<QueueItem>, Failure>;
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
}
