//! What a download client knows about the Usenet accounts behind it.
//!
//! A Usenet provider publishes nothing an operator can query: no quota endpoint, no
//! statement of what is left, no expiry to read. The client is where those facts
//! exist at all — the block that was bought is recorded there, and the bytes actually
//! pulled from each server are measured there, day by day. So an account's capacity is
//! read from the client that uses it, and what is not there is not knowable rather
//! than estimated.
//!
//! Everything below is either the operator's own record or the client's own
//! measurement, kept as the client holds it. The arithmetic that turns the two into
//! "how much is left" belongs above this seam, where it can be read and tested on its
//! own rather than hidden in a mapping.

use async_trait::async_trait;
use lemonfiber_manifest::Date;

use super::Failure;

/// An allowance recorded against an account, and the point it counts from.
///
/// The two travel together because neither means anything alone: a client measures
/// everything an account has ever pulled, so a block bought halfway through its life
/// is only readable against what had already gone when it was recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recorded {
    /// The allowance itself, in bytes.
    pub cap: u64,
    /// What the client had already pulled from the account when it was recorded.
    pub from: u64,
}

/// One Usenet account as its download client holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsenetAccount {
    /// What the operator calls it.
    pub name: String,
    /// Whether the client is using it at all — a disabled account downloads nothing,
    /// so nothing about its capacity is a fault.
    pub enabled: bool,
    /// The allowance recorded against it, where the operator recorded one.
    pub quota: Option<Recorded>,
    /// Everything the client has ever pulled from it.
    pub downloaded: u64,
    /// What it pulled on each day the client still holds a figure for.
    ///
    /// Per day rather than a client's own week or month total, because those reset on
    /// the calendar: read on a Monday morning, "this week" is an hour of downloading
    /// and projecting a block account from it would promise years it does not have.
    pub daily: Vec<(Date, u64)>,
    /// The day the subscription behind it ends, where the operator recorded one.
    pub expires_on: Option<Date>,
}

/// The Usenet accounts a download client is configured with.
#[async_trait]
pub trait UsenetAccounts: Send + Sync {
    /// Every account the client holds, with what it has recorded and measured for each.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the client could not be read.
    async fn accounts(&self) -> Result<Vec<UsenetAccount>, Failure>;
}

/// One indexer as the aggregator that queries it has it.
///
/// Counts rather than a limit, because an indexer states what it allows almost
/// nowhere: the caps in its capabilities document are results per query, not calls
/// per day. What the aggregator does know is how many times it asked, how many of
/// those failed, and whether it has since given up on it — which is the honest half
/// of the picture, and the half a cap would be judged against if one were published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexerUse {
    /// What the operator calls it.
    pub name: String,
    /// Whether the aggregator is querying it at all.
    pub enabled: bool,
    /// Searches made in the window asked for.
    pub queries: u64,
    /// How many of those searches it did not answer.
    pub failed_queries: u64,
    /// Releases taken from it in that window.
    pub grabs: u64,
    /// How many of those it did not hand over.
    pub failed_grabs: u64,
    /// When the aggregator will try it again, in its own words, where it has taken it
    /// out of rotation after repeated failures.
    pub rested_until: Option<String>,
}

/// The indexers an aggregator queries, and how they have been behaving.
#[async_trait]
pub trait Indexers: Send + Sync {
    /// Every indexer the aggregator holds, with its use since the start of `since`.
    ///
    /// Reading this costs the indexers nothing: the aggregator keeps its own counts,
    /// so asking it how much of an allowance has gone does not spend any of it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the aggregator could not be read.
    async fn indexers(&self, since: Date) -> Result<Vec<IndexerUse>, Failure>;
}
