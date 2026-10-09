//! What a member plays: the grant their client plays on, what a title is, how far
//! through it they got, and the progress a player reports back.
//!
//! Apart from the household's accounts because it is a different errand. That one is
//! who holds an account and what each may watch; this is the member sitting down to
//! watch, on a session that is their own. Every answer here is asked about one member's
//! account, so the media server applies that account's library access and age limit
//! before it answers, and nothing here applies them a second time.

use async_trait::async_trait;

use super::Failure;

/// The client every session a grant opens is signed in under.
///
/// What tells a device a grant signed in apart from one the member signed in to
/// themselves, and from the core's own sessions, so a lapse signs out the first and
/// leaves the rest alone.
pub const PLAYER: &str = "lemonfiber-player";

/// Playing what the household holds, on a member's own account.
#[async_trait]
pub trait Screening: Send + Sync {
    /// Open a session on `member`'s own account for one of their devices, and answer
    /// the token the device plays with; nothing where the server will not sign a
    /// device in by code.
    ///
    /// The session is the member's, with the member's limits, and nobody learns the
    /// member's password. It is opened under [`PLAYER`] and `device`, so asking again
    /// for the same device replaces the session it held, and a lapse can tell the
    /// sessions it opened from every other.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn signed_in(&self, member: &str, device: &str) -> Result<Option<String>, Failure>;

    /// What one title is, as this member's own account reads it, or nothing where
    /// their account may not see it.
    ///
    /// `None` names no member and reads it as an account with every library and no age
    /// limit would, which is what an invitation that chose nothing grants.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn title(&self, member: Option<&str>, id: &str) -> Result<Option<Title>, Failure>;

    /// What this member was part-way through, most recent first, `most` of them.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn part_way(&self, member: &str, most: u32) -> Result<Vec<PartWay>, Failure>;

    /// Record how far through one title this member is, as their own progress.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn progressed(&self, member: &str, id: &str, how_far: &HowFar) -> Result<(), Failure>;

    /// End every session one device holds, so it is refused from then on.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn sign_out(&self, device: &str) -> Result<(), Failure>;
}

/// What one title is, as a member's own account reads it.
///
/// What a person deciding whether to watch it wants, and nothing about how it is
/// stored: no file, no container, no bitrate.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Title {
    /// The title as the shelf lists it, with where it is served.
    #[serde(flatten)]
    pub held: Held,
    /// What it is about, where the server holds a description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    /// How long it runs, in whole minutes, where the server knows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minutes: Option<u32>,
    /// The genres the server files it under.
    pub genres: Vec<String>,
    /// The certificate it carries where the operator lives, where it carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,
    /// When it came out, as a calendar date, where the server knows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub released: Option<String>,
    /// A series' seasons, each with its episodes, in order. Empty for anything else.
    pub seasons: Vec<Season>,
}

/// One season of a series, with its episodes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Season {
    /// What the server tells it apart by.
    pub id: String,
    /// What it is called.
    pub name: String,
    /// Its number in the series, where it has one. Specials often have none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    /// Its episodes, in order, each with where it is served.
    pub episodes: Vec<Episode>,
}

/// One episode, with where it is served.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Episode {
    /// The episode as the shelf would list it, with where it is served.
    #[serde(flatten)]
    pub held: Held,
    /// Its number in the season, where it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    /// What happens in it, where the server holds a description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    /// How long it runs, in whole minutes, where the server knows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minutes: Option<u32>,
}

/// Something a member was part-way through, and how far.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct PartWay {
    /// The title or episode, with where it is served.
    #[serde(flatten)]
    pub held: Held,
    /// How far in they got, in whole seconds.
    pub position: u64,
    /// How long it runs, in whole seconds, where the server knows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<u64>,
}

/// How far through a title a member is, as a player reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HowFar {
    /// How far in, in whole seconds.
    pub position: u64,
    /// Whether they finished it.
    pub ended: bool,
}

/// One thing the household holds, as a member is shown it.
///
/// What a person recognises and nothing else. There is no file path, no container,
/// no bitrate and no library id: a member deciding what to watch is not choosing a
/// transcode, and a surface handed those would have to decide not to draw them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Held {
    /// The identifier the server tells it apart by, which is what asking to play one
    /// of them names.
    pub id: String,
    /// What it is called, in the words the server holds it under.
    pub title: String,
    /// The year it came out, where the server knows one. Absent rather than guessed:
    /// two films share a title far more often than they share a title and a year.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    /// Which of the kinds this product deals in it is.
    pub medium: Medium,
    /// Where it is served at the guarded front door, or why it is not.
    #[serde(flatten)]
    pub at: Located,
    /// What the server holds for it, which is what decides which locations it gets.
    #[serde(skip)]
    #[schemars(skip)]
    pub holds: Holds,
}

/// What the media server holds for one item: its pictures, and whether it streams.
///
/// Read by the adapter and turned into locations by the core, which alone knows where
/// the front door is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Holds {
    /// Whether it has a poster.
    pub poster: bool,
    /// Whether it has a backdrop.
    pub backdrop: bool,
    /// Whether it is something that plays, rather than something that holds what plays.
    pub plays: bool,
}

/// Where one item is served at the guarded front door, or why it is not.
///
/// Every location is built by the core from the household address the stack publishes
/// for the media server, so a client never puts one together.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Located {
    /// Where its poster is served, where it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster: Option<String>,
    /// Where its backdrop is served, where it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop: Option<String>,
    /// Where it streams from, where it plays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_from: Option<String>,
    /// The certificate the door presents, which a client pins, beside any location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub door: Option<Pinned>,
    /// Why no location is stated, where none is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unlocated: Option<String>,
}

/// The certificate a guarded door presents, as a client pins it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Pinned {
    /// SHA-256 over the certificate's DER encoding, lower-case hex.
    pub fingerprint: String,
}

/// The kinds of thing a household holds.
///
/// Named rather than passed through as the server's own word, because a surface
/// drawing "Series" against one server and "tvshow" against another would be
/// rendering a detail of which server this household runs.
///
/// `Medium` rather than `Kind`, `Holding` or `Sort`: this product already calls the two
/// request services a [`crate::media::Kind`], a request's suspension a
/// [`crate::service::asking::Holding`], and what one line of a manifest is a
/// `uninstall::Sort` — and one word meaning two things in one vocabulary is how a
/// reader comes to trust the wrong one. The contract flattens every type name into one
/// namespace, so a clash there is a clash for anything reading it by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Medium {
    /// One film.
    Film,
    /// A television series, rather than one episode of one.
    Series,
    /// One episode of a series.
    Episode,
    /// Something the server holds that is neither, and is not hidden for that.
    Other,
}
