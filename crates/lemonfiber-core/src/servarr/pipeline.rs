//! The Servarr fragment of an item's journey — reading one \*arr's library, history
//! and queue for a trace. It is the same [`Servarr`](super::Servarr) client the
//! provisioning adapter is built on; only the reads a trace needs live here, kept apart
//! from the writes that wire the stack together so the two concerns grow separately.

use async_trait::async_trait;
use serde::Deserialize;

use super::Servarr;
use crate::ports::http::Method;
use crate::ports::service::{Failure, FoundItem, Pipeline, QueueItem, StuckItem, TraceEvent};
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
            "/history?page=1&pageSize={}&sortKey=date&sortDirection=descending&{}={id}",
            crate::trace::HISTORY_HORIZON,
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
                crate::trace::Outcome::of_event(&record.event_type).map(|outcome| TraceEvent {
                    outcome,
                    at: record.date,
                })
            })
            .collect())
    }

    async fn item_queue(&self, kind: Kind, id: i64) -> Result<Option<QueueItem>, Failure> {
        let records = self.queue_pages("").await?;
        let mut found = false;
        let mut stage: Option<Stage> = None;
        let mut stuck = false;
        for record in records.iter().filter(|record| record.is_for(kind, id)) {
            found = true;
            // Being in the queue at all means at least downloading, even where the state is
            // unrecognised; the furthest of the item's records is what shows.
            if let Some(reached) = Stage::of_queue_state(&record.tracked_download_state) {
                stage = Some(stage.map_or(reached, |far| far.max(reached)));
            }
            stuck |= super::is_stuck(&record.tracked_download_status);
        }
        if !found {
            return Ok(None);
        }
        Ok(Some(QueueItem {
            stage: stage.unwrap_or(Stage::Downloading),
            stuck,
        }))
    }

    async fn stuck_items(&self, kind: Kind) -> Result<Vec<StuckItem>, Failure> {
        // The queue is read with the series and movie included so each stuck record names
        // the item a trace searches by, not the raw release. An item with no title to
        // trace by is left out rather than linked to a search that cannot find it.
        let records = self
            .queue_pages("&includeSeries=true&includeMovie=true")
            .await?;
        Ok(records
            .iter()
            .filter(|record| super::is_stuck(&record.tracked_download_status))
            .filter_map(|record| {
                record.item_title(kind).map(|title| StuckItem {
                    title,
                    stage: Stage::of_queue_state(&record.tracked_download_state)
                        .unwrap_or(Stage::Downloading),
                })
            })
            .collect())
    }
}

impl Servarr {
    /// Read every record in the queue, across its pages. A busy stack's queue can run to
    /// more than one page, so an item sitting past the first would read as absent — the
    /// service's own total bounds the walk. `query` appends extra parameters, such as the
    /// includes that embed each record's item title.
    async fn queue_pages(&self, query: &str) -> Result<Vec<super::QueueRecord>, Failure> {
        let mut records = Vec::new();
        let mut page = 1;
        loop {
            let path = format!("/queue?page={page}&pageSize={QUEUE_PAGE}{query}");
            let response = self.probe(&self.request(Method::Get, &path, None)).await?;
            let queue: super::QueueResource = self
                .endpoint
                .decode(&response, "the queue could not be read")?;
            let on_this_page = queue.records.len();
            let total = queue.total_records;
            records.extend(queue.records);
            if on_this_page < QUEUE_PAGE || page * QUEUE_PAGE >= total {
                break;
            }
            page += 1;
        }
        Ok(records)
    }
}

/// How many queue records are read per page — a generous page so most stacks answer in
/// one request, walked further only where the service's total says there is more.
const QUEUE_PAGE: usize = 200;

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
