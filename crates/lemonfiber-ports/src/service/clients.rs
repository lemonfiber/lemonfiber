//! The download clients and root folders a media service is told about.
//!
//! Everything that describes where content is fetched to and filed under, and the reads
//! that confirm what a service already holds.

use super::{Duration, Failure};
use crate::media::Kind;
use async_trait::async_trait;

/// The protocol a download client is reached in: the name of the API a curator speaks
/// to it, as the download client declares it.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct Protocol(pub String);

/// How a service proves itself: a single API key, or a username and its password.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Credential {
    /// A single API key.
    ApiKey(String),
    /// A username and its password.
    UserPass {
        /// The account name.
        username: String,
        /// Its password.
        password: String,
    },
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiKey(_) => formatter.write_str("ApiKey(..)"),
            Self::UserPass { username, .. } => formatter
                .debug_struct("UserPass")
                .field("username", username)
                .finish_non_exhaustive(),
        }
    }
}

/// The category a download is filed under, named after the media the requesting
/// application manages.
///
/// Each curator names the field after the media it files, so it travels with the
/// client rather than being assumed by the writer.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Category {
    /// The field the target application names its category.
    pub field: String,
    /// The value a download is filed under.
    pub value: String,
}

/// A download client, as one service needs to be told about another.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct DownloadClient {
    /// The name the operator will see in the service's own interface.
    pub name: String,
    /// The host the service should reach it on.
    pub host: String,
    /// The port it listens on.
    pub port: u16,
    /// The protocol it is reached in.
    pub protocol: Protocol,
    /// How the service authenticates to it.
    pub credential: Credential,
    /// The category the requesting application files its downloads under.
    pub category: Category,
}

/// A download client a service already holds, with the identifier it gave it.
///
/// Read back so a client already registered can be told from an absent one —
/// matched by the endpoint it reaches, the host and port, rather than by its
/// label, so a differently-named but equivalent client is not duplicated — and so
/// a later undo names exactly the one created.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RegisteredClient {
    /// The identifier the service assigned.
    pub id: String,
    /// The host the client is reached on.
    pub host: String,
    /// The port it listens on.
    pub port: u16,
    /// The category the client currently files under, where the service reports
    /// one — read back so an operator's change to it can be seen and preserved
    /// rather than reverted. Absent where the service names no category field.
    pub category: Option<Category>,
}

/// How a download client the service holds answered its reachability test —
/// the service's own verdict on whether the client it connects to is working,
/// keyed by the id the service assigned it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ClientProbe {
    /// The id of the client tested, matching a [`RegisteredClient::id`].
    pub id: String,
    /// Whether the client answered — `true` where the service reached it,
    /// `false` where the test failed.
    pub reachable: bool,
    /// What the service said when the test failed, where it said anything —
    /// carried so a warning can name why the client did not answer.
    pub detail: Option<String>,
}

/// One of the quality profiles a service holds, as the request service needs to
/// name it.
///
/// Both halves are carried because the request service wants both: the identifier
/// it will send, and the name it will show. Reading the name back rather than
/// assuming it means an operator who renamed a profile still sees their own word.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct QualityProfile {
    /// The identifier the service assigned.
    pub id: u32,
    /// What the operator calls it.
    pub name: String,
}

/// Where the request service reaches a curator: a host, a port, and the path it answers
/// under there.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    /// The host.
    pub host: String,
    /// The port.
    pub port: u16,
    /// The path the curator answers under at that host: empty where it answers at the
    /// root, and a route of its own where something in front of it answers for
    /// several.
    pub base: String,
}

/// A curator the request service hands requests of one kind to.
///
/// Everything the request service needs to reach it and to file what it fetches:
/// which kind it fetches, where it is, how to authenticate, and which profile and
/// folder to use.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct FulfilmentTarget {
    /// The name the operator will see in the request service's own interface.
    pub name: String,
    /// Where the request service should reach it.
    pub at: Endpoint,
    /// Where the request service may hold it from before, reached another way. A
    /// target held there is the same curator, and is moved here in place rather than
    /// registered a second time.
    pub moved_from: Option<Endpoint>,
    /// The key the request service authenticates with.
    pub key: String,
    /// The kind of video it fetches, which selects the request service's list for it.
    pub kind: Kind,
    /// The quality profile requests are fetched at.
    pub profile: QualityProfile,
    /// Where what it fetches is filed.
    pub folder: String,
}

/// A curator the request service already holds, with the identifier it gave it.
///
/// Read back so one already registered can be told from an absent one — matched by
/// the endpoint it reaches rather than by its label, the way a download client is,
/// so an operator who renamed it is not handed a duplicate.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RegisteredTarget {
    /// The identifier the request service assigned.
    pub id: String,
    /// Where it is reached.
    pub at: Endpoint,
    /// The key the request service presents there.
    pub key: String,
    /// The kind of video the list it is in fetches.
    pub kind: Kind,
}

/// Where a service should file the media it imports.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RootFolder {
    /// The path inside the container.
    pub path: String,
    /// Which media type it holds.
    pub media_type: String,
}

/// A root folder a service already holds, with the identifier it gave it.
///
/// Read back so an absent connection can be told from one already made — matched
/// by path, not by any label — and so a later undo names exactly the one created.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RegisteredFolder {
    /// The identifier the service assigned.
    pub id: String,
    /// The path it holds.
    pub path: String,
}

/// One active download a client is working, normalised to what the dashboard
/// shows — the point at which each client's own idea of progress becomes the
/// same three figures.
///
/// The protocol is not carried here: the gatherer sets it from the client it asked
/// rather than trusting each client to name it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Download {
    /// What is being downloaded.
    pub name: String,
    /// How far along, from zero to a hundred.
    pub progress: u8,
    /// The current speed in bytes per second, or `None` where the client reported
    /// none — kept apart from a reported zero, which is a download that is stalled
    /// rather than one whose speed is unknown.
    pub speed: Option<u64>,
    /// The time left, or `None` where the client gives none because it is stalled
    /// or cannot estimate one.
    pub eta: Option<Duration>,
    /// The bytes still to be written to disk for this download, or `None` where the
    /// client reports no figure — kept apart from a reported zero, which is a
    /// download already complete rather than one whose size is unknown. Summed
    /// across a stack's clients, this is the committed content the free-space
    /// projection weighs against what the volume has left.
    pub remaining: Option<u64>,
}

/// Reading a download client's active transfers for the dashboard.
///
/// Like [`Queues`], a read-only telemetry port kept off the wiring [`Client`]: the
/// dashboard asks a download client what it is moving right now, a question
/// seeding never needs, so the fakes that stand in for wiring need not answer it.
#[async_trait]
pub trait Transfers: Send + Sync {
    /// The downloads the client is working right now — each one's name, progress,
    /// speed and time left, the figures the dashboard shows without opening the
    /// client's own web UI.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the client is unreachable or refuses.
    async fn transfers(&self) -> Result<Vec<Download>, Failure>;
}

/// One completed download the client is still holding.
///
/// The three things a decision about reclaiming it turns on and nothing else: what
/// it is called, so it can be matched to what is on disk and to what a service is
/// waiting for; what it occupies; and what standing removing it would cost. A
/// client with no notion of ratio — Usenet has none to have — has nothing to
/// answer here, which is why this is a port of its own rather than another method
/// every download client would have to pretend to implement.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Seeded {
    /// What the client calls it, which is what both sides call it.
    pub name: String,
    /// What it occupies, as the client reports its size.
    pub bytes: u64,
    /// What it has uploaded against what it downloaded, in hundredths.
    ///
    /// Whole hundredths rather than a fraction, because what is done with this is
    /// shown to a person and compared for equality between two runs, and a
    /// fraction cannot be compared for equality. No decision made on a ratio is
    /// finer than a hundredth.
    pub ratio: u32,
}

/// Reading what a download client is still seeding.
///
/// Apart from [`Transfers`] because it is a different question with a different
/// answer: that one asks what is arriving and this asks what has arrived and is
/// still being given back. Only a torrent client has an answer, and a port that
/// bundled the two would oblige every Usenet client to say something about seeding.
#[async_trait]
pub trait Seeding: Send + Sync {
    /// The completed downloads the client is still holding, each with what it
    /// occupies and the ratio it has earned.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the client is unreachable or refuses.
    async fn seeding(&self) -> Result<Vec<Seeded>, Failure>;
}

/// Reading a service's queue for the dashboard.
///
/// A port of its own, not a method on [`Client`], because it is a read-only
/// telemetry capability the dashboard uses rather than part of the wiring shape
/// seeding drives — keeping it apart means the fakes that stand in for a service
/// being wired need not answer for a question they are never asked.
#[async_trait]
pub trait Queues: Send + Sync {
    /// The service's queue: how deep it is, and the items on the page read.
    ///
    /// Both, because they answer different questions and neither substitutes for
    /// the other. The depth is the service's own count and is authoritative — a
    /// queue deeper than one page would otherwise be under-reported by however
    /// much did not fit. The items are what a stall is categorised from, since a
    /// number cannot say which ones or why.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn queue(&self) -> Result<Queue, Failure>;
}

/// A service's queue as one read of it found it.
#[derive(
    Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Queue {
    /// How many items the service says it has, whatever fitted on the page.
    pub total: usize,
    /// The items on the page that was read.
    pub items: Vec<Queued>,
}

impl Queue {
    /// Whether the service answered with nothing at all.
    ///
    /// Distinct from a queue that could not be read, which is a `Failure` and
    /// never reaches here: an empty queue is a working stack with nothing to do,
    /// and rendering the two alike is how an operator comes to believe a service
    /// is idle when it is unreachable.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.total == 0 && self.items.is_empty()
    }
}

/// One thing in a service's queue, in the service's own terms.
///
/// Deliberately close to what the API returns: interpreting it is
/// the queue model's, and a port that decided what counted as stuck would put
/// the judgement in the one place a test cannot reach it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Queued {
    /// What the service calls it — the name the download client also knows it by,
    /// which is what correlates the two sides.
    pub title: String,
    /// The service's own word for how it is tracking: `ok`, `warning`, `error`.
    pub status: String,
    /// The service's own word for what stage it is at, such as `importPending`.
    pub state: String,
    /// What the service said went wrong, where it said anything. The blocking
    /// cause, in the words of the thing that refused — a permission denial in an
    /// import log is worth more than any interpretation of it.
    pub message: Option<String>,
    /// The download client's own identifier for it, where the service knows one.
    /// A surer correlation than the title, which either side may have rewritten.
    pub download_id: Option<String>,
    /// How many times the service has grabbed this item since it last imported
    /// it — at least the one that put it here.
    ///
    /// Counted per item rather than per release, because a loop commonly grabs a
    /// different release each time for the same episode, and counted only since
    /// the last import, which is what separates a loop from an upgrade. A history
    /// nobody could read leaves it at one: a count that could not be taken is not
    /// a loop.
    pub grabs: u32,
}

impl Queued {
    /// Whether the service considers this one to have stopped progressing.
    #[must_use]
    pub fn is_stuck(&self) -> bool {
        self.status.eq_ignore_ascii_case("warning") || self.status.eq_ignore_ascii_case("error")
    }
}

/// How deep a service's queue is, and how much of it is stuck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueDepth {
    /// How many items are queued in total.
    pub total: usize,
    /// How many of them are stuck — warning or error — rather than progressing.
    pub stuck: usize,
}

impl QueueDepth {
    /// The depth a read of the queue amounts to.
    ///
    /// The total is the service's own; the stuck count is read from the items that
    /// came back, so a queue deeper than one page reports its true depth with a
    /// stuck count from the page — an under-count of what is wrong rather than an
    /// invented one.
    #[must_use]
    pub fn of(queue: &Queue) -> Self {
        Self {
            total: queue.total,
            stuck: queue.items.iter().filter(|item| item.is_stuck()).count(),
        }
    }
}

#[cfg(test)]
mod tests;
