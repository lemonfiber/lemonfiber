//! What a member plays: the grant their client plays on, what a title is, how far
//! through it they got, and the progress a player reports back.
//!
//! Apart from the household's accounts because it is a different errand. That one is
//! who holds an account and what each may watch; this is the member sitting down to
//! watch, on a session that is their own. Every answer here is asked about one member's
//! account, so the media server applies that account's library access and age limit
//! before it answers, and nothing here applies them a second time.

use async_trait::async_trait;

use super::{Failure, Playback};

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
    async fn title(&self, member: Option<&str>, id: &str) -> Result<Option<ItemDetail>, Failure>;

    /// What this member was part-way through, most recent first, `most` of them.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn part_way(&self, member: &str, most: u32) -> Result<Vec<ItemProgress>, Failure>;

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

    /// What this member may watch, as the media server answers it for them.
    ///
    /// **Asked for that member, never filtered for them.** The server holds the age
    /// limit, the library access and the blocked kinds, and answering about one
    /// account is a thing it already does — so what comes back is what they may see
    /// because the server said so, whoever's credential carried the question. A read
    /// taken about the household and narrowed here would be a second copy of every one
    /// of those rules, able to disagree with the first on the day either moved.
    ///
    /// Sorted and bounded by the server rather than here: a household library is
    /// larger than a screen, and deciding which part of it to ask for is the caller's
    /// errand rather than this one's.
    ///
    /// `None` names no member and reads what an account with every library and no age
    /// limit holds, which is what an invitation that chose nothing grants. It is asked
    /// about no account, so it carries nothing of anybody's.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn holdings(&self, member: Option<&str>, most: u32) -> Result<Vec<Item>, Failure>;

    /// What the media server is playing now: one entry a session playing something,
    /// for one account where `member` names its identifier, or for every account.
    ///
    /// Asked of the server each time rather than kept, because what is playing is the
    /// fact most likely to have changed since anybody last asked. A session signed in
    /// and playing nothing is not listed: it is a device, not somebody watching.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn playing(&self, member: Option<&str>) -> Result<Vec<Playback>, Failure>;
}

/// One title, as a member's own account reads it on the server.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ItemDetail {
    /// The item.
    pub item: Item,
    /// What it is about, where the server holds a description.
    pub overview: Option<String>,
    /// How long it runs, in whole minutes, where the server knows.
    pub minutes: Option<u32>,
    /// The genres the server files it under.
    pub genres: Vec<String>,
    /// The certificate it carries where the operator lives, where it carries one.
    pub certificate: Option<String>,
    /// When it came out, as a calendar date, where the server knows.
    pub released: Option<String>,
    /// A series' seasons, each with its episodes, in order. Empty for anything else.
    pub seasons: Vec<SeasonDetail>,
}

/// One season of a series, with its episodes, as the server holds it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct SeasonDetail {
    /// What the server tells it apart by.
    pub id: String,
    /// What it is called.
    pub name: String,
    /// Its number in the series, where it has one.
    pub number: Option<u32>,
    /// Its episodes, in order.
    pub episodes: Vec<EpisodeDetail>,
}

/// One episode, as the server holds it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct EpisodeDetail {
    /// The item.
    pub item: Item,
    /// Its number in the season, where it has one.
    pub number: Option<u32>,
    /// What happens in it, where the server holds a description.
    pub overview: Option<String>,
    /// How long it runs, in whole minutes, where the server knows.
    pub minutes: Option<u32>,
}

/// Something a member was part-way through, and how far, as the server holds it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ItemProgress {
    /// The item.
    pub item: Item,
    /// How far in they got, in whole seconds.
    pub position: u64,
    /// How long it runs, in whole seconds, where the server knows.
    pub length: Option<u64>,
}

/// How far through a title a member is, as a player reports it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct HowFar {
    /// How far in, in whole seconds.
    pub position: u64,
    /// Whether they finished it.
    pub ended: bool,
}

/// One thing the household holds, as the server holds it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// The identifier the server tells it apart by.
    pub id: String,
    /// What it is called, in the words the server holds it under.
    pub title: String,
    /// The year it came out, where the server knows one.
    pub year: Option<u16>,
    /// Which of the kinds this product deals in it is.
    pub medium: Medium,
    /// What the server holds for it.
    pub holds: Holds,
}

/// What the media server holds for one item: its pictures, and whether it streams.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Holds {
    /// Whether it has a poster.
    pub poster: bool,
    /// Whether it has a backdrop.
    pub backdrop: bool,
    /// Whether it is something that plays, rather than something that holds what plays.
    pub plays: bool,
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema, serde::Deserialize,
)]
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
