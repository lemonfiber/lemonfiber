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
            .field("token", &withheld(self.token.as_ref()))
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

/// The live TV channels one member may watch, each with the programme on it now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ChannelsReport {
    /// The member this was asked for, by the name they are known by.
    pub member: String,
    /// The identifier the media server files them under.
    pub id: String,
    /// Each channel their own account may watch whose programme now is within their
    /// limits. Empty where their account may not watch live TV.
    pub channels: Vec<crate::ports::service::Channel>,
    /// Whether this was a rehearsal: what would have happened, with none of it done.
    pub rehearsed: bool,
}

/// A channel's live stream opened for a member, where its bytes are served at the
/// guarded front door, each on a pass for that live stream and that member.
#[derive(Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct TunedReport {
    /// The channel, by the identifier the channels list gives it.
    pub channel: String,
    /// The member it is opened for, by the name they are known by.
    pub member: String,
    /// The live stream a renewal or a stop names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<String>,
    /// Where the tuner's own stream is served, carrying its pass.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct: Option<String>,
    /// Where a transcoded playlist of it is served, carrying its pass.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcoded: Option<String>,
    /// SHA-256 over the certificate the door presents, lower-case hex, which a client
    /// pins.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// When the passes lapse, in whole seconds since the Unix epoch. A client asks for
    /// new ones before then.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lapses: Option<u64>,
    /// Why nothing is located, where nothing is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unlocated: Option<String>,
    /// Whether this was a rehearsal: what would have happened, with none of it done.
    pub rehearsed: bool,
}

/// Written by hand so no pass is ever in what is debugged or logged.
impl std::fmt::Debug for TunedReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TunedReport")
            .field("channel", &self.channel)
            .field("member", &self.member)
            .field("stream", &self.stream)
            .field("direct", &withheld(self.direct.as_ref()))
            .field("transcoded", &withheld(self.transcoded.as_ref()))
            .field("fingerprint", &self.fingerprint)
            .field("lapses", &self.lapses)
            .field("unlocated", &self.unlocated)
            .field("rehearsed", &self.rehearsed)
            .finish()
    }
}

/// Where one title plays from in a member's browser, at the stack's front door, on a
/// pass sealing a session on their own account.
#[derive(Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct PlayReport {
    /// The title or episode, by the identifier the shelf lists it under.
    pub id: String,
    /// The member it plays as, by the name they are known by.
    pub member: String,
    /// The playlist it plays from at the front door, carrying its pass.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_from: Option<String>,
    /// When the pass lapses, in whole seconds since the Unix epoch. A browser hands it
    /// back for a new one before then.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lapses: Option<u64>,
    /// Why nothing is located, where nothing is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unlocated: Option<String>,
    /// Whether this was a rehearsal: what would have happened, with none of it done.
    pub rehearsed: bool,
}

/// Written by hand so no pass is ever in what is debugged or logged.
impl std::fmt::Debug for PlayReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayReport")
            .field("id", &self.id)
            .field("member", &self.member)
            .field("stream_from", &withheld(self.stream_from.as_ref()))
            .field("lapses", &self.lapses)
            .field("unlocated", &self.unlocated)
            .field("rehearsed", &self.rehearsed)
            .finish()
    }
}

/// What a secret is written as where it is debugged: that one is there, and nothing of it.
fn withheld(secret: Option<&String>) -> Option<&'static str> {
    secret.map(|_| WITHHELD)
}

/// What stands for a secret where it is debugged.
pub const WITHHELD: &str = "<withheld>";
