//! What a member sits down to watch: one title, what they were part-way through, the
//! grant their client plays on, and the progress a player reports.
//!
//! Each is the media server's answer about the member's own account, located at the
//! guarded front door by the core, so a client draws and plays what it is handed and
//! decides nothing about who may see what.

use serde::Serialize;

use super::held::Held;

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

/// One title, as the member it was asked for may see it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct TitleReport {
    /// The member this was asked for, by the name they are known by.
    pub member: String,
    /// The identifier the media server files them under.
    pub id: String,
    /// The title, with where it and each of its episodes is served.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Title>,
    /// Whether this was a rehearsal: what would have happened, with none of it done.
    pub rehearsed: bool,
}

/// What one member was part-way through, most recent first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct PartWayReport {
    /// The member this was asked for, by the name they are known by.
    pub member: String,
    /// The identifier the media server files them under.
    pub id: String,
    /// Each title or episode, how far in, and where it is served.
    pub part_way: Vec<PartWay>,
    /// Whether it could be read at all. An empty list and an unread one are different
    /// answers, and what could not be read is said in `findings`.
    pub available: bool,
    /// What is worth saying about this read, in the words its reader would use.
    pub findings: Vec<String>,
    /// Whether this was a rehearsal: what would have happened, with none of it done.
    pub rehearsed: bool,
}

/// A grant to play on a member's own account, the token the device plays with, and how
/// long it lasts.
#[derive(Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct GrantReport {
    /// The member the device now plays as, by the name they are known by.
    pub member: String,
    /// Whether a session was opened for the device. A rehearsal opens none.
    pub granted: bool,
    /// The token the device presents at the guarded front door, as
    /// `Authorization: Bearer <token>`. Answered once, here, and kept by nothing in the
    /// core; absent where nothing was granted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// The last day the grant holds unless the member's client speaks to the core
    /// before then, as `YYYY-MM-DD`.
    pub lasts_until: String,
    /// Whether this was a rehearsal: what would have happened, with none of it done.
    pub rehearsed: bool,
}

/// Written by hand so the token is never in what is debugged or logged.
impl std::fmt::Debug for GrantReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GrantReport")
            .field("member", &self.member)
            .field("granted", &self.granted)
            .field("token", &self.token.as_ref().map(|_| "<withheld>"))
            .field("lasts_until", &self.lasts_until)
            .field("rehearsed", &self.rehearsed)
            .finish()
    }
}

/// A player's report of how far a member got, as the media server now holds it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct WatchedReport {
    /// The title or episode, by the identifier the shelf lists it under.
    pub id: String,
    /// How far in, in whole seconds, as recorded.
    pub position: u64,
    /// Whether it was recorded as finished.
    pub ended: bool,
    /// Whether this was a rehearsal: what would have happened, with none of it done.
    pub rehearsed: bool,
}
