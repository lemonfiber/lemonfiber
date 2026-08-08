//! The download clients and root folders a media service is told about.
//!
//! Everything that describes where content is fetched to and filed under, and the reads
//! that confirm what a service already holds.

use super::{Duration, Failure};
use async_trait::async_trait;

/// Which download client an entry is, selecting the field schema the Servarr app
/// files it under — see the download-client contract in the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    /// `SABnzbd` — a Usenet client.
    Sabnzbd,
    /// `qBittorrent` — a torrent client.
    Qbittorrent,
}

/// How a download client proves itself, which differs by client — a single API
/// key for one, a username and its password for the other.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// The category a download is filed under, named after the media the requesting
/// application manages.
///
/// The field is not shared across applications — Sonarr names it `tvCategory`,
/// Radarr `movieCategory`, Lidarr `musicCategory` — so it travels with the client
/// rather than being assumed by the writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    /// The field the target application names its category.
    pub field: String,
    /// The value a download is filed under.
    pub value: String,
}

/// A download client, as one service needs to be told about another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadClient {
    /// The name the operator will see in the service's own interface.
    pub name: String,
    /// The host the service should reach it on.
    pub host: String,
    /// The port it listens on.
    pub port: u16,
    /// Which client it is, selecting the field schema.
    pub kind: ClientKind,
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Where a service should file the media it imports.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
/// The protocol is not carried here: it is intrinsic to which client answered —
/// a torrent from qBittorrent, a Usenet download from `SABnzbd` — so the gatherer
/// sets it from the target it asked rather than trusting each client to name it.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Reading a service's queue for the dashboard.
///
/// A port of its own, not a method on [`Client`], because it is a read-only
/// telemetry capability the dashboard uses rather than part of the wiring shape
/// seeding drives — keeping it apart means the fakes that stand in for a service
/// being wired need not answer for a question they are never asked.
#[async_trait]
pub trait Queues: Send + Sync {
    /// How deep the service's queue is, and how much of it is stuck — the numbers
    /// the dashboard shows without opening the service's own web UI.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn queue(&self) -> Result<QueueDepth, Failure>;
}

/// How deep a service's queue is, and how much of it is stuck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueDepth {
    /// How many items are queued in total.
    pub total: usize,
    /// How many of them are stuck — warning or error — rather than progressing.
    pub stuck: usize,
}
