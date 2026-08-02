//! The Servarr fragment of an item's journey — reading one \*arr's library, history
//! and queue for a trace. It is the same [`Servarr`](super::Servarr) client the
//! provisioning adapter is built on; only the reads a trace needs live here, kept apart
//! from the writes that wire the stack together so the two concerns grow separately.

use async_trait::async_trait;
use serde::Deserialize;

use super::Servarr;
use crate::ports::http::Method;
use crate::ports::service::{Failure, FoundItem, Pipeline, QueueItem, TraceEvent};
use crate::recyclarr::Kind;
use crate::trace::Stage;

#[async_trait]
impl Pipeline for Servarr {
    async fn find_items(&self, kind: Kind, term: &str) -> Result<Vec<FoundItem>, Failure> {
        let response = self
            .probe(&self.request(Method::Get, &format!("/{}", kind.library_endpoint()), None))
            .await?;
        let items: Vec<LibraryItem> = self
            .endpoint
            .decode(&response, "the library could not be read")?;
        let needle = term.to_lowercase();
        Ok(items
            .into_iter()
            .filter(|item| item.title.to_lowercase().contains(&needle))
            .map(|item| FoundItem {
                id: item.id,
                title: item.title,
                monitored: item.monitored,
            })
            .collect())
    }

    async fn item_history(&self, kind: Kind, id: i64) -> Result<Vec<TraceEvent>, Failure> {
        let path = format!(
            "/history?page=1&pageSize=100&sortKey=date&sortDirection=descending&{}={id}",
            kind.history_filter()
        );
        let response = self.probe(&self.request(Method::Get, &path, None)).await?;
        let page: HistoryPage = self
            .endpoint
            .decode(&response, "the history could not be read")?;
        Ok(page
            .records
            .into_iter()
            .filter_map(|record| {
                Stage::of_event(&record.event_type).map(|stage| TraceEvent {
                    stage,
                    at: record.date,
                })
            })
            .collect())
    }

    async fn item_queue(&self, kind: Kind, id: i64) -> Result<Option<QueueItem>, Failure> {
        let response = self
            .probe(&self.request(Method::Get, "/queue?pageSize=200", None))
            .await?;
        let queue: super::QueueResource = self
            .endpoint
            .decode(&response, "the queue could not be read")?;
        let mut records = queue
            .records
            .iter()
            .filter(|record| record.is_for(kind, id))
            .peekable();
        if records.peek().is_none() {
            return Ok(None);
        }
        // A series may have several episodes queued; the item's furthest download stage
        // and whether any of its records is stuck are what the trace reads. Being in the
        // queue at all means at least downloading, even where the state is unrecognised.
        let stage = records
            .clone()
            .filter_map(|record| Stage::of_queue_state(&record.tracked_download_state))
            .max()
            .unwrap_or(Stage::Downloading);
        let stuck = records.any(|record| super::is_stuck(&record.tracked_download_status));
        Ok(Some(QueueItem { stage, stuck }))
    }
}

/// One library item — a series or a film — as the service lists it, matched by title to
/// find something to trace.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryItem {
    id: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    monitored: bool,
}

/// A page of history: the events on it, newest first.
#[derive(Deserialize)]
struct HistoryPage {
    #[serde(default)]
    records: Vec<HistoryRecord>,
}

/// One history event — its type names what happened, the date names when.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryRecord {
    #[serde(default)]
    event_type: String,
    #[serde(default)]
    date: String,
}
