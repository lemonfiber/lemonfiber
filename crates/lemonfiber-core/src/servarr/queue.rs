//! Reading a service's queue, and how many times it has re-grabbed what is in it.
//!
//! The queue says what is in flight. It does not say how it got there, and the
//! difference matters: an item fetched over and over is a system quietly spending
//! bandwidth and Usenet allowance on an import that keeps failing, and it looks
//! entirely normal from the queue alone — one record, downloading, nothing wrong.
//!
//! So the history is read alongside it. **Counted per item rather than per
//! release**, because a loop commonly changes release: the service grabs one
//! thing for an episode, fails, and grabs a different thing for the same episode.
//! Counting by release name would see two unrelated items and never find it.
//!
//! And counted only **since the last successful import**, which is the difference
//! between a loop and an upgrade. An episode grabbed again after it was imported
//! is a better copy replacing a worse one — the system working — and reporting
//! that as a fault would flag every upgrade on the machine.

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::Deserialize;

use super::{QueueRecord, QueueResource, Servarr};
use crate::ports::http::Method;
use crate::ports::service::{Failure, Queue, Queued, Queues};
use crate::trace::Outcome;

/// How many history events are read to count re-grabs.
///
/// A recency window rather than a time one, because that is what the endpoint
/// offers. Its consequence is stated where it lands: a loop whose grabs have
/// scrolled past this reads as fewer grabs, never as more, so the check
/// under-reports rather than inventing one.
const HISTORY_PAGE: usize = 200;

#[async_trait]
impl Queues for Servarr {
    async fn queue(&self) -> Result<Queue, Failure> {
        // A generous page is asked for so the stuck count is read from the whole
        // queue rather than the default first page; the total is the service's own
        // count, independent of the page.
        let response = self
            .probe(&self.request(Method::Get, "/queue?pageSize=200", None))
            .await?;
        let queue: QueueResource = self
            .endpoint
            .decode(&response, "the queue could not be read")?;
        let grabs = self.grabs().await;
        Ok(Queue {
            total: queue.total_records,
            items: queue
                .records
                .into_iter()
                .map(|record| queued(record, &grabs))
                .collect(),
        })
    }
}

impl Servarr {
    /// How many times each item has been grabbed since it was last imported.
    ///
    /// Empty where the history could not be read, and deliberately not an error:
    /// the queue is the answer being given, and losing it because a second read
    /// failed would turn a missing count into a missing queue. A count nobody
    /// could read is not a loop.
    ///
    /// Unreachable and unreadable are one expression here rather than two, because
    /// they are one behaviour: whatever stopped the history arriving, what is known
    /// about re-grabs is nothing.
    async fn grabs(&self) -> BTreeMap<i64, u32> {
        let path = format!(
            "/history?page=1&pageSize={HISTORY_PAGE}&sortKey=date&sortDirection=descending"
        );
        self.probe(&self.request(Method::Get, &path, None))
            .await
            .ok()
            .and_then(|response| {
                self.endpoint
                    .decode::<HistoryPage>(&response, "the history could not be read")
                    .ok()
            })
            .map(|page| counted(&page.records))
            .unwrap_or_default()
    }
}

/// The grabs standing against each item, from history newest first.
///
/// An import ends the count for that item: what came before it was a copy that
/// arrived, and grabbing again afterwards is an upgrade rather than a retry.
fn counted(records: &[HistoryRecord]) -> BTreeMap<i64, u32> {
    let mut grabs: BTreeMap<i64, u32> = BTreeMap::new();
    let mut settled: BTreeMap<i64, bool> = BTreeMap::new();
    for record in records {
        let Some(item) = record.item() else {
            continue;
        };
        if *settled.get(&item).unwrap_or(&false) {
            continue;
        }
        match Outcome::of_event(&record.event_type) {
            Some(Outcome::Grabbed) => *grabs.entry(item).or_insert(0) += 1,
            Some(Outcome::Imported) => {
                settled.insert(item, true);
            }
            Some(Outcome::DownloadFailed | Outcome::Removed) | None => {}
        }
    }
    grabs
}

/// One queue record as the port reports it.
///
/// The service's own words are carried through rather than interpreted here: what
/// counts as stuck, and which category a stall belongs to, is a judgement, and a
/// judgement made in an adapter is one no test can reach without a network. The
/// grab count is not a judgement — it is how many records the service holds.
fn queued(record: QueueRecord, grabs: &BTreeMap<i64, u32>) -> Queued {
    let grabbed = record
        .item()
        .and_then(|item| grabs.get(&item).copied())
        .unwrap_or(0);
    Queued {
        title: record.title.unwrap_or_default(),
        status: record.tracked_download_status,
        state: record.tracked_download_state,
        // The service reports a single message and a list of them; the list is
        // where an import failure explains itself, so it leads.
        message: record
            .status_messages
            .into_iter()
            .flat_map(|message| message.messages)
            .next()
            .or(record.error_message)
            .filter(|message| !message.trim().is_empty()),
        download_id: record.download_id.filter(|id| !id.is_empty()),
        // At least the one that put it here. A history that could not be read, or
        // that has scrolled past this item's grabs, leaves the count at the single
        // fetch the queue record itself is evidence of.
        grabs: grabbed.max(1),
    }
}

/// A page of history: the events on it, newest first.
#[derive(Deserialize)]
struct HistoryPage {
    #[serde(default)]
    records: Vec<HistoryRecord>,
}

/// One history event — its type names what happened, and the item it happened to
/// is what a re-grab is counted against.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryRecord {
    #[serde(default)]
    event_type: String,
    #[serde(default)]
    episode_id: Option<i64>,
    #[serde(default)]
    movie_id: Option<i64>,
}

impl HistoryRecord {
    /// The item this event happened to, whichever kind of service filed it.
    const fn item(&self) -> Option<i64> {
        match self.episode_id {
            Some(episode) => Some(episode),
            None => self.movie_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{counted, HistoryRecord};

    /// One history event of the given type, for the given episode.
    fn event(event_type: &str, episode: i64) -> HistoryRecord {
        HistoryRecord {
            event_type: event_type.to_owned(),
            episode_id: Some(episode),
            movie_id: None,
        }
    }

    #[test]
    fn the_same_item_grabbed_again_and_again_is_counted() {
        // Counted per item rather than per release: a loop commonly grabs a
        // different release each time for the same episode, and counting by name
        // would see three unrelated items and never find it.
        let history = [
            event("grabbed", 7),
            event("downloadFailed", 7),
            event("grabbed", 7),
            event("downloadFailed", 7),
            event("grabbed", 7),
        ];
        assert_eq!(counted(&history).get(&7), Some(&3));
    }

    #[test]
    fn an_upgrade_is_not_a_loop() {
        // Newest first: grabbed once since the import, and imported before that.
        // An episode re-grabbed after it arrived is a better copy replacing a
        // worse one — the system working. Counting the older grabs too would flag
        // every upgrade on the machine.
        let history = [
            event("grabbed", 7),
            event("downloadFolderImported", 7),
            event("grabbed", 7),
            event("grabbed", 7),
        ];
        assert_eq!(counted(&history).get(&7), Some(&1));
    }

    #[test]
    fn one_item_looping_says_nothing_about_another() {
        let history = [
            event("grabbed", 7),
            event("grabbed", 8),
            event("grabbed", 7),
            event("grabbed", 7),
        ];
        let counts = counted(&history);
        assert_eq!(counts.get(&7), Some(&3));
        assert_eq!(counts.get(&8), Some(&1));
    }

    #[test]
    fn a_film_is_counted_the_same_way_as_an_episode() {
        // The film services file history by movie, and nothing here needs to know
        // which kind of service answered.
        let film = |event_type: &str| HistoryRecord {
            event_type: event_type.to_owned(),
            episode_id: None,
            movie_id: Some(11),
        };
        assert_eq!(
            counted(&[film("grabbed"), film("grabbed")]).get(&11),
            Some(&2)
        );
    }

    #[test]
    fn an_event_about_nothing_identifiable_is_passed_over() {
        // History carries events with no item on them at all. They are not grabs of
        // something unnamed; they are events this has no business counting.
        let anonymous = HistoryRecord {
            event_type: "grabbed".to_owned(),
            episode_id: None,
            movie_id: None,
        };
        assert!(counted(&[anonymous]).is_empty());
    }

    #[test]
    fn an_event_this_does_not_recognise_changes_nothing() {
        let history = [event("episodeFileRenamed", 7), event("grabbed", 7)];
        assert_eq!(counted(&history).get(&7), Some(&1));
    }

    #[test]
    fn an_empty_history_counts_nothing_rather_than_guessing() {
        assert!(counted(&[]).is_empty());
    }
}
