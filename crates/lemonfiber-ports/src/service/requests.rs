//! Setting up the request service, and reading what the household asked through it.
//!
//! Apart from [`Asking`](super::Asking) because that port answers about one person and
//! one request — what they may ask for, what they have left, what becomes of one thing
//! they asked for. This is the service itself: where it authenticates from, which \*arrs
//! it hands requests on to, who it knows about, and what it says to all of them at once.

use async_trait::async_trait;

use super::{Failure, FulfilmentTarget, RegisteredTarget};

/// One thing a household member asked for, as the request service records it.
///
/// The two statuses are carried as the service's own numbers rather than folded here:
/// what became of the request and what became of the media it asked for are separate
/// facts, and turning the pair into one word a member reads is a decision for the household
/// model above this, not for the code that reads them off the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HouseholdRequest {
    /// The number the request service files this request under, which is how one is
    /// named to it again when somebody rules on it.
    pub id: i64,
    /// When it was asked for, as the service timestamps it — what a request waiting
    /// on somebody is measured against, and what a counting period runs from.
    pub made: Option<String>,
    /// The member who asked, by the name the request service shows them under.
    pub member: String,
    /// Which service files the media — television or film — or `None` where the
    /// request service names a media type this build does not know.
    pub kind: Option<crate::media::Kind>,
    /// The id the \*arr filing this media knows it by, where the request service has
    /// handed it over yet. Nothing for a request still awaiting approval, which no
    /// \*arr has been told about — so the item cannot be named from the library, and
    /// is not claimed to be.
    pub item: Option<i64>,
    /// What became of the request, as the service numbers them.
    pub request_status: u8,
    /// What became of the media it asked for, as the service numbers them.
    pub media_status: u8,
}

/// What one member may ask for on the request service.
///
/// Only the half that bears on what a household chose. Everything else about the
/// account — what they are called, what they may watch — is the media server's to say,
/// and a second copy here would be a copy able to disagree with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requesting {
    /// The identifier this service tells them apart by.
    pub id: String,
    /// Whether what they ask for arrives without anybody approving it.
    ///
    /// True is the state a restriction has to undo: it is the whole of how a limit on
    /// watching and a lack of limit on requesting come apart.
    pub approves_own: bool,
}

/// A request manager's identity setup and the household's own requests — Seerr,
/// configured to authenticate its household against the media server rather than
/// against accounts of its own.
#[async_trait]
pub trait Requests: Send + Sync {
    /// Whether it has already been initialised — the gate that never re-points a
    /// running instance's identity source and so keeps its existing sign-ins.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn initialized(&self) -> Result<bool, Failure>;

    /// Point authentication at the media server reached at `server_url`, signing
    /// in as `username` with `password` — which on the first call also creates the
    /// owner from that account.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn configure_identity(
        &self,
        username: &str,
        password: &str,
        server_url: &str,
    ) -> Result<(), Failure>;

    /// Sign in through the media server as `username` with `password`, leaving the
    /// session the later reads are made under.
    ///
    /// Signing in is what [`Requests::configure_identity`] does first; this is that step
    /// on its own, for a read that must not also finish somebody's setup.
    ///
    /// **Where the media server is, is not named here**, and that is the difference
    /// between the two. A service that has been pointed at one already knows where it
    /// is, and naming it again is an attempt to point it somewhere — which it refuses,
    /// because moving a household's identity source out from under them is not a thing
    /// a sign-in should be able to do. So this opens a session on a service that is
    /// already set up, and [`Requests::configure_identity`] is the one that sets it up.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn sign_in(&self, username: &str, password: &str) -> Result<(), Failure>;

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

    /// The \*arrs it already hands requests to, by the endpoint each reaches.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn fulfilment_targets(&self) -> Result<Vec<RegisteredTarget>, Failure>;

    /// Hand it an \*arr to fulfil requests through.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn add_fulfilment_target(&self, target: &FulfilmentTarget) -> Result<(), Failure>;
}

/// Whether the request service reaches the household, and about what.
///
/// The occasions are a set, carried as the bit field the service keeps them in. It
/// is a number here rather than a list of named events because that is the shape the
/// service reads and writes, and translating it twice — once out, once back — would
/// be two places for the set to lose a member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Telling {
    /// Whether it will send anything at all.
    pub enabled: bool,
    /// Which occasions it sends on.
    pub occasions: u32,
}
