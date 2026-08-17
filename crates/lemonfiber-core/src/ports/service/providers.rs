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

use std::time::{Duration, SystemTime};

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

/// What the client's last exchange with an account showed.
///
/// Its records say what an account has pulled; this says what happened when the client
/// last actually spoke to it, which no total can show — a hundred gigabytes pulled last
/// week is no evidence the account answers this morning. So the two are kept apart, and
/// only this half is a reply from the provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// Connections the client has open and ready to it.
    ///
    /// The one proof of a working account there is: a connection is ready only once the
    /// provider has taken the credential on it, which no record of past bytes can show.
    pub ready: u64,
    /// How many connections the client is set to open to it.
    pub configured: u64,
    /// Whether the client still has it in rotation.
    ///
    /// The difference between an account nobody is downloading through and one that has
    /// stopped answering, which no other figure here can tell apart: an idle client holds
    /// no connections to a perfectly good account. A client drops an account it cannot
    /// use and picks it up again itself once it works, so being out of rotation is the
    /// client's own current verdict rather than a memory of an old failure.
    pub serving: bool,
    /// The last trouble the client recorded against it, in the words it recorded.
    ///
    /// Kept verbatim, the provider's own message and all: what those words amount to is
    /// a judgment, and a mapping is not where judgments belong.
    pub trouble: Option<String>,
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
    /// What the client's last exchange with it showed, where it has one to report. An
    /// account the client is not set to use has none: nothing has been asked of it.
    pub standing: Option<Standing>,
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

/// What an indexer allows, as the aggregator was told it.
///
/// An indexer publishes its allowance almost nowhere — the caps in its capabilities
/// document are results per query, not calls per day — so the figure that exists is the
/// one the operator typed into the aggregator after reading their own subscription. That
/// makes it exactly the shape a Usenet block takes: the allowance is recorded in the
/// client, the use is measured by the client, and asking either costs the provider
/// nothing.
///
/// The window travels with the caps because a cap means nothing without one, and because
/// the aggregator counts against a *rolling* window: a count taken from midnight would
/// report headroom that is not there every morning, which is the same calendar trap the
/// download client's weekly totals set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Searches allowed in the window, where the operator recorded a cap.
    pub queries: Option<u64>,
    /// Grabs allowed in the window, where the operator recorded a cap.
    pub grabs: Option<u64>,
    /// How far back the window reaches from now.
    pub window: Duration,
}

/// One indexer as the aggregator that queries it has it.
///
/// Counts and, where the operator recorded them, the caps those counts are judged
/// against. What the aggregator always knows is how many times it asked, how many of
/// those failed, and whether it has since given up on the indexer; what it knows only
/// sometimes is what the subscription behind it allows.
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
    /// What it allows and over how long, where the operator recorded either.
    pub limits: Option<Limits>,
    /// When the oldest search still inside the window was made.
    ///
    /// A reset falls out of this and nothing else: a rolling window frees up the moment
    /// its oldest call ages out of it. A total has no times in it, so a count alone can
    /// say an allowance is spent and never say when it comes back.
    pub searched_from: Option<SystemTime>,
    /// When the oldest grab still inside the window was made — the same, for the other
    /// allowance, which runs out on its own schedule.
    pub grabbed_from: Option<SystemTime>,
}

/// The indexers an aggregator queries, and how they have been behaving.
#[async_trait]
pub trait Indexers: Send + Sync {
    /// Every indexer the aggregator holds, with its use over the window its own caps are
    /// counted in, taken back from `now`.
    ///
    /// A moment rather than a day, because the caps are counted over a window that rolls:
    /// one that began at midnight would report an allowance as barely touched every
    /// morning, however much of it went overnight. An indexer capped by the hour cannot
    /// be counted over a date at all.
    ///
    /// Reading this costs the indexers nothing: the aggregator keeps its own counts,
    /// so asking it how much of an allowance has gone does not spend any of it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the aggregator could not be read.
    async fn indexers(&self, now: SystemTime) -> Result<Vec<IndexerUse>, Failure>;
}
