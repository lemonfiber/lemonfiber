//! Setting up the request service, and reading what the household asked through it.
//!
//! Apart from [`Asking`](super::Asking) because that port answers about one person and
//! one request — what they may ask for, what they have left, what becomes of one thing
//! they asked for. This is the service itself: where it authenticates from, which curators
//! it hands requests on to, who it knows about, and what it says to all of them at once.

use std::collections::BTreeSet;

use async_trait::async_trait;

use super::{Credential, Endpoint, Failure, FulfilmentTarget, Protocol, RegisteredTarget};
use crate::media::Kind;

/// One thing a household member asked for, as the request service records it.
///
/// What became of the request and what became of the media it asked for are two
/// statuses; the household model turns the pair into the one word a member reads.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct HouseholdRequest {
    /// The number the request service files this request under, which is how one is
    /// named to it again when somebody rules on it.
    pub id: i64,
    /// When it was asked for, as the service timestamps it — what a request waiting
    /// on somebody is measured against, and what a counting period runs from.
    pub made: Option<String>,
    /// The member who asked, by the name the request service shows them under.
    ///
    /// A name for saying, not for telling people apart: the request service lets
    /// anybody change the name it shows them by.
    pub member: String,
    /// The id the media server files the member who asked under, where the request
    /// service records one — which is how a request is told to be somebody's.
    pub member_id: Option<String>,
    /// Which service files the media — television or film — or `None` where the
    /// request service names a media type this build does not know.
    pub kind: Option<crate::media::Kind>,
    /// The id the curator filing this media knows it by, where the request service has
    /// handed it over yet. Nothing for a request still awaiting approval, which no
    /// curator has been told about — so the item cannot be named from the library, and
    /// is not claimed to be.
    pub item: Option<i64>,
    /// When the media it asked for arrived on the media server, as the request service
    /// timestamps it, or nothing until it is there.
    pub arrived: Option<String>,
    /// The identifier the media server holds that media under, as the request service
    /// records it, or nothing until it is there.
    pub shelf_id: Option<String>,
    /// What became of the request, or `None` where the service reports a status this
    /// contract does not name.
    pub request_status: Option<RequestStatus>,
    /// What became of the media it asked for, or `None` likewise.
    pub media_status: Option<MediaStatus>,
}

/// What became of one request itself.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RequestStatus {
    /// Nobody has approved or refused it yet.
    Pending,
    /// Approved: the services were asked for it.
    Approved,
    /// Turned down.
    Declined,
    /// The attempt to fetch it failed.
    Failed,
    /// Finished with: where the media stands is the answer now.
    Completed,
}

/// What became of the media one request asked for.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum MediaStatus {
    /// Nothing is known about it yet.
    Unknown,
    /// Known and waiting.
    Pending,
    /// Being fetched.
    Processing,
    /// Some of it is here.
    PartlyAvailable,
    /// All of it is here.
    Available,
    /// It was here and has been removed.
    Deleted,
}

/// The identity source a request service signs the household in through: where it is,
/// the protocol it is spoken to in, and the credential that administers it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct IdentitySource {
    /// Where the request service reaches it.
    pub at: String,
    /// The protocol it is spoken to in.
    pub protocol: Protocol,
    /// The credential that administers it.
    pub credential: Credential,
}

/// What one member may ask for on the request service.
///
/// Only the half that bears on what a household chose. Everything else about the
/// account — what they are called, what they may watch — is the media server's to say,
/// and a second copy here would be a copy able to disagree with it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Requesting {
    /// The identifier this service tells them apart by.
    pub id: String,
    /// Whether what they ask for arrives without anybody approving it.
    ///
    /// True is the state a restriction has to undo: it is the whole of how a limit on
    /// watching and a lack of limit on requesting come apart.
    pub approves_own: bool,
}

/// A request service's identity setup and the household's own requests, configured
/// to authenticate its household against the identity source rather than against
/// accounts of its own.
#[async_trait]
pub trait Requests: Send + Sync {
    /// Whether it has already been initialised — the gate that never re-points a
    /// running instance's identity source and so keeps its existing sign-ins.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn initialized(&self) -> Result<bool, Failure>;

    /// Sign the household in through `source`, as the account its credential names, and
    /// finish setting up — which on the first call also creates the owner from that
    /// account.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses, and
    /// [`Failure::Unsupported`] for a protocol it does not sign in through.
    async fn configure_identity(&self, source: &IdentitySource) -> Result<(), Failure>;

    /// Whether the service answers to the key this client carries, as its owner.
    ///
    /// The service's own key is what every read and write after its setup carries, so
    /// the media server's administrator password passes through it only once, on the
    /// sign-in that sets it up.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses the key.
    async fn answers(&self) -> Result<(), Failure>;

    /// Every request the household has made, across its members.
    ///
    /// Read as the owner, whose session sees the whole household: the members
    /// themselves have no way to run this, so the one account lemonfiber holds a
    /// credential for asks on their behalf.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn requests(&self) -> Result<Vec<HouseholdRequest>, Failure>;

    /// Give the request service an account for each of these media-server members.
    ///
    /// The link an invitation owes: the account exists on the media server from the
    /// moment somebody is invited, and this is what makes the same person known to
    /// the service they ask through.
    ///
    /// **Sending somebody it already knows is not an error and does nothing** — the
    /// service skips a member it already holds. So this is safe to call with everybody
    /// on every run, and a link that could not be made while the service was down is
    /// completed by the next run rather than by anything remembered in between.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn link_members(&self, members: &[String]) -> Result<(), Failure>;

    /// The account this service holds for a media-server member, where it holds one.
    ///
    /// `None` where it holds none — a member who has never signed in here is somebody
    /// this service has never heard of, which is **nothing to revoke** rather than a
    /// failure to revoke something.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn member_for(&self, media_server_id: &str) -> Result<Option<String>, Failure>;

    /// What one member may ask for here, by the media server's own identifier.
    ///
    /// `None` where this service holds no account for them, which is a member who has
    /// never signed in here rather than a read that failed.
    ///
    /// Wanted because a limit on what somebody may *watch* says nothing about what they
    /// may *ask for*, and the two disagreeing is the gap parental controls exist to
    /// close: a child who cannot watch something but can pull it into the library is a
    /// child whose parents' setting did half a job.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn requesting(&self, media_server_id: &str) -> Result<Option<Requesting>, Failure>;

    /// Make what this member asks for wait for somebody to approve it.
    ///
    /// **The narrowest thing this service can be told about a restricted member.** It
    /// has no notion of a content rating, so there is no limit here to mirror the media
    /// server's — what there is instead is the difference between a request that lands
    /// in the library unseen and one that an adult sees first. Taking the approval off
    /// leaves everything else about the account exactly as it was.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn approval_first(&self, id: &str) -> Result<(), Failure>;

    /// Take that account away, and with it everything it asked for.
    ///
    /// **This destroys their requests**, which is the service's own behaviour and not a
    /// choice made here: it removes them by hand so that a title still waiting goes back
    /// to being unrequested rather than being left pointing at nobody. Anything shown to
    /// an operator before this runs has to say so.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn remove_member(&self, id: &str) -> Result<(), Failure>;

    /// What the request service will tell the household about, as it stands.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn telling(&self) -> Result<Telling, Failure>;

    /// Set what it tells them about.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn tell(&self, telling: &Telling) -> Result<(), Failure>;

    /// The curators it already hands requests to, by the endpoint each reaches.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn fulfilment_targets(&self) -> Result<Vec<RegisteredTarget>, Failure>;

    /// Hand it a curator to fulfil requests through.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn add_fulfilment_target(&self, target: &FulfilmentTarget) -> Result<(), Failure>;

    /// Point a target it holds at `at`, presenting `key`, leaving everything else
    /// about it — its name, profile, folder and the rest — as it holds them.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable, refuses, or no longer holds `held`.
    async fn move_fulfilment_target(
        &self,
        held: &RegisteredTarget,
        at: &Endpoint,
        key: &str,
    ) -> Result<(), Failure>;

    /// Ask it to reach the curator of `kind` at `at` presenting `key`, the way its own
    /// settings test does, from wherever it runs.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable, or could not reach the curator.
    async fn test_fulfilment_target(
        &self,
        kind: Kind,
        at: &Endpoint,
        key: &str,
    ) -> Result<(), Failure>;

    /// Where it reaches the media server for everything after a sign-in, and with what.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn media_server_link(&self) -> Result<MediaServerLink, Failure>;

    /// Reach the media server at `at` with `key` from now on, leaving everything else
    /// about the connection as it holds it. The service proves the pair against the
    /// media server before it keeps it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable, refuses, or the media server does
    /// not answer to the pair.
    async fn link_media_server(&self, at: &Endpoint, key: &str) -> Result<(), Failure>;
}

/// Where the request service reaches the media server, and the key it presents there.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct MediaServerLink {
    /// Where.
    pub at: Endpoint,
    /// The key.
    pub key: String,
}

/// Whether the request service reaches the household, and about what.
#[derive(
    Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Telling {
    /// Whether it will send anything at all.
    pub enabled: bool,
    /// Which occasions it sends on.
    pub occasions: BTreeSet<Occasion>,
    /// Whether it also sends on occasions this contract does not name.
    pub others: bool,
}

/// One occasion the request service tells the household about.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Occasion {
    /// A request was received and waits on somebody.
    Received,
    /// A request was approved.
    Approved,
    /// What was asked for arrived.
    Arrived,
    /// What was asked for could not be got.
    Failed,
    /// A request was turned down.
    Declined,
    /// A request was approved by the household's own policy, with nobody asked.
    ApprovedByPolicy,
}

impl Occasion {
    /// Every occasion.
    pub const ALL: [Self; 6] = [
        Self::Received,
        Self::Approved,
        Self::Arrived,
        Self::Failed,
        Self::Declined,
        Self::ApprovedByPolicy,
    ];
}
