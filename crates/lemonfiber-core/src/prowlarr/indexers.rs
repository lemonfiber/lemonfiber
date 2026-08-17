//! How the indexers behind the stack have been behaving, and what they allow.
//!
//! Two halves of one answer. The counts are the aggregator's own, kept because it made
//! every call itself, so reading them costs the indexers nothing. The allowance they are
//! judged against is the operator's own record: an indexer publishes almost nothing about
//! what it allows — the limits in a capabilities document are results per query, not calls
//! per day — so the only figure that exists anywhere is the one they typed in after
//! reading their subscription.
//!
//! Everything is counted the way the aggregator counts it, because that is what the caps
//! were set against. Its window rolls rather than starting at midnight, and a search a
//! person asked for and a search a feed poll made are one allowance to the indexer while
//! the aggregator keeps them in columns of its own.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::Deserialize;

use super::Prowlarr;
use crate::instant;
use crate::ports::service::{Failure, IndexerUse, Indexers, Limits};

/// The epoch as a service writes it — the window start for a clock so far out that no
/// window can be taken back from it, which is a stack whose time is wrong rather than an
/// indexer worth reporting on.
const EPOCH: &str = "1970-01-01T00:00:00";

/// An indexer as Prowlarr lists it.
///
/// The resource declares a `status` too, and Prowlarr never fills it in — the mapper
/// that builds this answer leaves it null, and the standing of an indexer lives at its
/// own endpoint. Reading it here would report every indexer as having never failed.
#[derive(Deserialize)]
struct IndexerResource {
    id: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    enable: bool,
    /// Every setting, flattened into named values — which is where the caps live, under
    /// the names their nesting gives them.
    #[serde(default)]
    fields: Vec<SettingField>,
}

/// One of an indexer's settings, as Prowlarr flattens them.
///
/// A setting the operator never filled in comes back named, with a null value, rather
/// than being left out — so an absent cap and a cap of nothing are the same answer here,
/// and both mean there is nothing to judge use against.
#[derive(Deserialize)]
struct SettingField {
    #[serde(default)]
    name: String,
    #[serde(default)]
    value: Option<serde_json::Value>,
}

/// The setting that names how many searches an indexer allows in its window.
const QUERY_CAP: &str = "baseSettings.queryLimit";

/// The setting that names how many grabs it allows in the same window.
const GRAB_CAP: &str = "baseSettings.grabLimit";

/// The setting that names how long that window is.
const CAP_WINDOW: &str = "baseSettings.limitsUnit";

/// The window setting's value for an hourly allowance; anything else is a daily one,
/// which is what Prowlarr itself falls back to.
const HOURLY: u64 = 1;

/// How long a daily allowance is counted over.
const DAILY_WINDOW: Duration = Duration::from_secs(24 * 60 * 60);

/// How long an hourly one is.
const HOURLY_WINDOW: Duration = Duration::from_secs(60 * 60);

/// One indexer's standing, from the endpoint that does fill it in.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexerStatusResource {
    indexer_id: i64,
    #[serde(default)]
    disabled_till: Option<String>,
}

/// The counts Prowlarr keeps for each indexer over a window.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexerStatsResource {
    #[serde(default)]
    indexers: Vec<IndexerCounts>,
}

/// One indexer's counts, split the way Prowlarr splits them.
///
/// A search a person asked for and a search a feed poll made are counted in separate
/// columns here, and against the *same* allowance everywhere else — so the two are added
/// back together. Reading only the first would leave every automatic poll out of the
/// figure, and those are most of the traffic on a stack that is working.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexerCounts {
    indexer_id: i64,
    #[serde(default)]
    number_of_queries: i64,
    #[serde(default)]
    number_of_rss_queries: i64,
    #[serde(default)]
    number_of_grabs: i64,
    #[serde(default)]
    number_of_failed_queries: i64,
    #[serde(default)]
    number_of_failed_rss_queries: i64,
    #[serde(default)]
    number_of_failed_grabs: i64,
}

/// One indexer's first call of each kind still inside its window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct FirstCalls {
    searched: Option<SystemTime>,
    grabbed: Option<SystemTime>,
}

/// A history entry: which indexer, when, and what was done.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryRecord {
    indexer_id: i64,
    #[serde(default)]
    date: String,
    #[serde(default)]
    event_type: EventType,
}

/// What a history entry was, however the aggregator's version happens to write it.
///
/// Named in some versions and numbered in others, and a reading that bet on one would
/// count nothing at all under the other — which looks exactly like an indexer nobody is
/// querying rather than like a misread.
#[derive(Deserialize, Default)]
#[serde(untagged)]
enum EventType {
    Named(String),
    Numbered(i64),
    #[default]
    Unwritten,
}

impl EventType {
    /// Whether this is a search, counted against the search allowance — both the kind a
    /// person asks for and the kind a feed poll makes.
    fn is_search(&self) -> bool {
        matches!(self, Self::Named(name) if name == "indexerQuery" || name == "indexerRss")
            || matches!(self, Self::Numbered(2 | 3))
    }

    /// Whether this is a grab, counted against the grab allowance.
    fn is_grab(&self) -> bool {
        matches!(self, Self::Named(name) if name == "releaseGrabbed")
            || matches!(self, Self::Numbered(1))
    }
}

impl Prowlarr {
    /// The counts each indexer has run up since `start`.
    async fn counts_since(&self, start: &str) -> Result<IndexerStatsResource, Failure> {
        self.read(
            &format!("/indexerstats?startDate={start}"),
            "the indexer counts could not be read",
        )
        .await
    }

    /// The first call of each kind each indexer made inside its own window.
    ///
    /// Read only where an allowance is recorded, because it is only wanted to say when one
    /// frees up: a window that rolls frees up as its oldest call ages out, and the
    /// aggregator publishes that moment nowhere. It costs the indexers nothing either way —
    /// this is the aggregator's own log of calls it already made.
    ///
    /// One read covers every indexer, so it reaches back as far as the longest window in
    /// use and each indexer's own cutoff is applied to what comes back. Without that, an
    /// indexer counted by the hour on a stack that also counts one by the day would be
    /// dated from a call made yesterday — and told its allowance returns a day late.
    async fn first_calls(
        &self,
        now: SystemTime,
        listed: &[IndexerResource],
    ) -> Result<BTreeMap<i64, FirstCalls>, Failure> {
        let Some(reach) = longest_capped_window(listed) else {
            return Ok(BTreeMap::new());
        };
        let history: Vec<HistoryRecord> = self
            .read(
                &format!("/history/since?date={}", window_start(now, reach)),
                "the indexer history could not be read",
            )
            .await?;
        let windows = cutoffs(now, listed);
        let mut first: BTreeMap<i64, FirstCalls> = BTreeMap::new();
        for record in history {
            let (Some(at), Some(cutoff)) =
                (instant::read(&record.date), windows.get(&record.indexer_id))
            else {
                continue;
            };
            if at < *cutoff {
                continue;
            }
            let calls = first.entry(record.indexer_id).or_default();
            if record.event_type.is_search() {
                calls.searched = Some(calls.searched.map_or(at, |first| first.min(at)));
            } else if record.event_type.is_grab() {
                calls.grabbed = Some(calls.grabbed.map_or(at, |first| first.min(at)));
            }
        }
        Ok(first)
    }
}

#[async_trait]
impl Indexers for Prowlarr {
    async fn indexers(&self, now: SystemTime) -> Result<Vec<IndexerUse>, Failure> {
        let listed: Vec<IndexerResource> = self
            .read("/indexer", "the indexers could not be read")
            .await?;
        let standing: Vec<IndexerStatusResource> = self
            .read("/indexerstatus", "the indexer standings could not be read")
            .await?;
        // One read per window in use rather than one per indexer: an aggregator whose
        // indexers are all counted by the day asks once, and the most any stack can need
        // is the two windows there are.
        let mut counted: BTreeMap<u64, IndexerStatsResource> = BTreeMap::new();
        for window in windows_in_use(&listed) {
            let stats = self.counts_since(&window_start(now, window)).await?;
            counted.insert(window.as_secs(), stats);
        }
        // The times are only wanted to date a reset, so they are only read where there is
        // an allowance for anything to reset against.
        let first = self.first_calls(now, &listed).await?;
        Ok(listed
            .into_iter()
            .map(|indexer| {
                let limits = limits_of(&indexer);
                let counts = counted
                    .get(&window_of(&indexer).as_secs())
                    .and_then(|stats| {
                        stats
                            .indexers
                            .iter()
                            .find(|counts| counts.indexer_id == indexer.id)
                    });
                let rested = standing
                    .iter()
                    .find(|status| status.indexer_id == indexer.id)
                    .and_then(|status| status.disabled_till.clone());
                let calls = first.get(&indexer.id).copied().unwrap_or_default();
                indexer_use(indexer, counts, rested, limits, calls)
            })
            .collect())
    }
}

/// The start of `window` taken back from `now`, as a service reads a moment.
///
/// Both ends are clamped to the epoch, and for the same reason: a clock with less than a
/// window behind it, or one so far out that no calendar holds it, is a machine whose time
/// is wrong rather than a window worth refusing over. Asking from the beginning reads
/// every row there is instead of none, which is the harmless way to be wrong here.
fn window_start(now: SystemTime, window: Duration) -> String {
    let since = now.checked_sub(window).unwrap_or(UNIX_EPOCH);
    instant::written(since).unwrap_or_else(|| EPOCH.to_owned())
}

/// Every window the indexers being queried are counted over, without repeats.
///
/// Only the ones in use: an indexer nobody is querying has run nothing up, and asking for
/// a second window on its behalf is a read that answers nothing.
fn windows_in_use(listed: &[IndexerResource]) -> BTreeSet<Duration> {
    listed
        .iter()
        .filter(|indexer| indexer.enable)
        .map(window_of)
        .collect()
}

/// When each capped indexer's window began — the moment a call has to be no older than
/// for it to still count against that indexer's allowance.
fn cutoffs(now: SystemTime, listed: &[IndexerResource]) -> BTreeMap<i64, SystemTime> {
    listed
        .iter()
        .filter(|indexer| indexer.enable && limits_of(indexer).is_some())
        .map(|indexer| {
            (
                indexer.id,
                now.checked_sub(window_of(indexer)).unwrap_or(UNIX_EPOCH),
            )
        })
        .collect()
}

/// The longest window any queried indexer with a recorded cap is counted over, where any
/// has one — the reach the aggregator's log has to cover to date every reset there is.
fn longest_capped_window(listed: &[IndexerResource]) -> Option<Duration> {
    listed
        .iter()
        .filter(|indexer| indexer.enable && limits_of(indexer).is_some())
        .map(window_of)
        .max()
}

/// How long one indexer's allowance is counted over. A daily window is the aggregator's
/// own fallback, and the fallback here for the same reason: it is the one nearly every
/// subscription is sold in.
fn window_of(indexer: &IndexerResource) -> Duration {
    if setting(indexer, CAP_WINDOW) == Some(HOURLY) {
        HOURLY_WINDOW
    } else {
        DAILY_WINDOW
    }
}

/// What an indexer allows, where the operator recorded either allowance.
///
/// Nothing at all where neither is recorded: a window with no cap in it is not a limit
/// nobody reached, it is a limit nobody stated, and the two must not read alike.
fn limits_of(indexer: &IndexerResource) -> Option<Limits> {
    let queries = setting(indexer, QUERY_CAP);
    let grabs = setting(indexer, GRAB_CAP);
    (queries.is_some() || grabs.is_some()).then(|| Limits {
        queries,
        grabs,
        window: window_of(indexer),
    })
}

/// One numeric setting of an indexer, where it holds a number that can be a count.
fn setting(indexer: &IndexerResource, name: &str) -> Option<u64> {
    indexer
        .fields
        .iter()
        .find(|field| field.name == name)?
        .value
        .as_ref()?
        .as_u64()
}

/// One listed indexer joined to its counts, its standing, and what it allows.
///
/// An indexer the counts do not mention has not been asked for anything in the
/// window, which is a zero rather than a gap — and a zero is worth reporting: an
/// indexer nobody is querying is a different thing from one that is failing.
fn indexer_use(
    indexer: IndexerResource,
    counts: Option<&IndexerCounts>,
    rested_until: Option<String>,
    limits: Option<Limits>,
    calls: FirstCalls,
) -> IndexerUse {
    let count = |taken: fn(&IndexerCounts) -> i64| {
        counts.map_or(0, |counts| u64::try_from(taken(counts)).unwrap_or(0))
    };
    IndexerUse {
        name: indexer.name,
        enabled: indexer.enable,
        queries: count(|counts| counts.number_of_queries)
            .saturating_add(count(|counts| counts.number_of_rss_queries)),
        failed_queries: count(|counts| counts.number_of_failed_queries)
            .saturating_add(count(|counts| counts.number_of_failed_rss_queries)),
        grabs: count(|counts| counts.number_of_grabs),
        failed_grabs: count(|counts| counts.number_of_failed_grabs),
        rested_until,
        limits,
        searched_from: calls.searched,
        grabbed_from: calls.grabbed,
    }
}

#[cfg(test)]
mod tests {
    use super::{window_start, Duration, SystemTime, DAILY_WINDOW, EPOCH, UNIX_EPOCH};

    /// The moment a fixed clock reads, so a window taken back from it is the same every
    /// run: noon on a day the arithmetic can be checked by hand.
    fn noon() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_786_968_000)
    }

    #[test]
    fn a_window_is_taken_back_from_the_moment_it_is_asked_at() {
        assert_eq!(window_start(noon(), DAILY_WINDOW), "2026-08-16T12:00:00");
    }

    /// A clock with less than a window behind it has nothing to take one back from, and
    /// asking from the beginning reads every row there is rather than none.
    #[test]
    fn a_clock_with_no_room_behind_it_asks_from_the_beginning() {
        assert_eq!(window_start(UNIX_EPOCH, DAILY_WINDOW), EPOCH);
    }

    /// A clock so far out that no calendar holds it is a machine whose time is wrong,
    /// which is worth reading past rather than refusing over.
    #[test]
    fn a_clock_beyond_any_calendar_asks_from_the_beginning_too() {
        let absurd = UNIX_EPOCH + Duration::from_secs(3_000_000_000_000);
        assert_eq!(window_start(absurd, DAILY_WINDOW), EPOCH);
    }
}
