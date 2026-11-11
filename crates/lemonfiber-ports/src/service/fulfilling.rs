//! What a curator answers the request gate on a household's behalf: the titles it holds or
//! its catalogue knows, each by its catalogue reference, a request for one, a search, a
//! removal and what it is downloading.

use async_trait::async_trait;

use super::Failure;
use crate::media::{Kind, TitleKind};

/// A title as a curator holds it, or as its catalogue knows it where it holds none.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CuratedTitle {
    /// Its catalogue reference: its `TheTVDB` id for a series, its `TMDB` id for a film, and
    /// 0 for an album or a book.
    pub reference: i64,
    /// The curator's own id for it, where it holds it.
    pub id: Option<i64>,
    /// What it is called.
    pub title: String,
    /// The year it came out, where the catalogue gives one.
    pub year: Option<u32>,
    /// Whether the curator is monitoring it.
    pub monitored: bool,
    /// How many files of it the curator holds.
    pub files: u32,
    /// Each season of a series; none for a film.
    pub seasons: Vec<CuratedSeason>,
}

/// One season of a series a curator holds or knows.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CuratedSeason {
    /// Its number; season zero is where a curator files specials.
    pub number: u32,
    /// Whether the curator is monitoring it.
    pub monitored: bool,
    /// How many of its episodes have a file.
    pub files: u32,
    /// How many episodes it has.
    pub episodes: u32,
}

/// A household's request for one title, as the curator is asked to take it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct TitleRequest {
    /// The title's catalogue reference.
    pub reference: i64,
    /// The root folder a title it does not hold is filed under.
    pub root_folder: String,
    /// The quality profile a title it does not hold is fetched at.
    pub quality_profile: i64,
    /// The seasons of a series asked for; none for a film.
    pub seasons: Vec<u32>,
    /// Whether a series is numbered as anime is.
    pub anime: bool,
    /// Whether the seasons of a series that air later are monitored too.
    pub later_seasons: bool,
    /// Whether it searches for what it then wants.
    pub search: bool,
}

/// Where one download stands.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum DownloadState {
    /// Waiting for the client to start it.
    Queued,
    /// Held by the client.
    Paused,
    /// Coming down now.
    Downloading,
    /// Down, and being filed.
    Completed,
    /// Not progressing, as the curator tracks it.
    Warning,
    /// Given up on.
    Failed,
}

/// The episode one download is for.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct DownloadedEpisode {
    /// The season it is in.
    pub season: u32,
    /// Its number in that season.
    pub number: u32,
    /// What it is called.
    pub title: String,
}

/// One download a curator is tracking for a title it holds.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Downloading {
    /// The curator's own id for the title.
    pub title_id: i64,
    /// The release's name.
    pub release: String,
    /// Its size, in bytes.
    pub bytes: u64,
    /// What is left of it, in bytes.
    pub left: u64,
    /// When it is expected to complete, in seconds since the Unix epoch, where the client
    /// says.
    pub expected: Option<i64>,
    /// Where it stands.
    pub state: DownloadState,
    /// The download client's own id for it, where the curator knows one.
    pub download_id: Option<String>,
    /// The curator's id for its queue entry, the id `unqueue` takes, from minor 3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued: Option<i64>,
    /// The episode it is for, where it is for one.
    pub episode: Option<DownloadedEpisode>,
}

/// What a curator answers the request gate.
#[async_trait]
pub trait Fulfilling: Send + Sync {
    /// The title of `kind` its catalogue holds by `reference`, with its own id where it
    /// holds it, or none.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the curator is unreachable or refuses.
    async fn catalogued(&self, kind: Kind, reference: i64)
        -> Result<Option<CuratedTitle>, Failure>;

    /// Every title of `kind` it holds.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the curator is unreachable or refuses.
    async fn titles(&self, kind: TitleKind) -> Result<Vec<CuratedTitle>, Failure>;

    /// The title of `kind` it holds as `id`, or none.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the curator is unreachable or refuses.
    async fn held_title(&self, kind: Kind, id: i64) -> Result<Option<CuratedTitle>, Failure>;

    /// Take `request` for a title of `kind`: add it where it holds none, monitoring the
    /// seasons asked and no other; monitor it, those seasons and every unmonitored episode
    /// of them where it holds it, changing nothing else; then search where asked.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Refused`] where the root folder or the quality profile is not one
    /// it holds, and [`Failure`] when the curator is unreachable.
    async fn request(&self, kind: Kind, request: &TitleRequest) -> Result<CuratedTitle, Failure>;

    /// Search for what it wants of the title of `kind` it holds as `id`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Refused`] where it holds no such title, and [`Failure`] when the
    /// curator is unreachable.
    async fn search(&self, kind: TitleKind, id: i64) -> Result<(), Failure>;

    /// Remove the title of `kind` it holds as `id`, with its files, excluding nothing from a
    /// later import.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Refused`] where it holds no such title, and [`Failure`] when the
    /// curator is unreachable.
    async fn remove(&self, kind: Kind, id: i64) -> Result<(), Failure>;

    /// What it is downloading of `kind`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the curator is unreachable or refuses.
    async fn downloading(&self, kind: TitleKind) -> Result<Vec<Downloading>, Failure>;

    /// Remove the queue entry of `kind` it holds as `id` from the download client holding
    /// it, blocklisting its release where `blocklist`; the release's fingerprint.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Refused`] where it holds no such entry, and [`Failure`] when the
    /// curator or its download client is unreachable.
    async fn unqueue(&self, kind: TitleKind, id: i64, blocklist: bool) -> Result<String, Failure>;

    /// Lift the blocklist entry of the release fingerprinted `release` for the title of
    /// `kind` it holds as `id`, search the indexers for the title and grab the release whose
    /// fingerprint matches.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Refused`] where it holds no such title, and [`Failure`] when the
    /// curator is unreachable.
    async fn grab(&self, kind: TitleKind, id: i64, release: &str) -> Result<Grabbed, Failure>;
}

/// What grabbing a release again came to.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Grabbed {
    /// An indexer offered the release, and it was sent to the download client.
    Grabbed,
    /// No indexer offers it any more.
    Gone,
}
