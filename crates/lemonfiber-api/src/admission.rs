//! The second way in, and the only door that opens without a token.
//!
//! Everything else on this surface is guarded by a secret minted at start and
//! printed on the terminal that started the process. That answers one question —
//! *is this the machine's own operator* — and it answers it by having been printed
//! there, which is the population loopback already answers for. It is no use at all
//! to somebody holding a phone, and being reachable from a phone is the whole case
//! for ever offering this surface beyond loopback.
//!
//! So the password is exchanged, **once**, for a session. Once because verifying
//! one is deliberately expensive, which is the point of how it is stored; and a
//! credential re-sent on every request is a credential with more chances to leak.
//! What comes back travels in the same header the per-run token does, so the guard
//! reads one credential header and a client holds one thing rather than two.
//!
//! **This is the one route that answers a request carrying no token**, and it has
//! to be: a caller with a password and nothing else is exactly who it is for. What
//! it does not lose is the other half of the guard — the request must still say it
//! came from where this server is listening, so a page the operator happens to be
//! visiting cannot post guesses here with their browser. It sits under the same one
//! layer every other route sits under, which is what keeps the surface's fallback
//! guarded and what makes a route added beside it guarded by having been added; the
//! layer names this one path and nothing else, and a test holds the whole surface to
//! exactly one path being reachable without a token.
//!
//! A wrong password answers `401`, and nothing else does. It is answered where the
//! password was offered, so a client reading it knows the password it just sent is
//! the thing to change. A session this run no longer admits answers `403` with
//! every other refusal of who is asking, and what tells a client that signing in
//! again would help is the refusal's code rather than its status.

pub mod admitted;
pub mod attempts;
mod claiming;
pub mod keyed;
pub mod keyring;
mod proving;
pub mod remembered;
pub mod sessions;

use claiming::signed_in;
use sessions::Opened;

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::post;
use axum::{Extension, Json, Router};
use lemonfiber_core::admission::{self as credential, Credential};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::model::{kind, Envelope};
use lemonfiber_core::ports::random::Random;
use lemonfiber_core::ports::service::{Household, Signed};
use serde::Deserialize;

use crate::guard::{host_is_here, origin_is_here, Arrived, Binding, Token, TOKEN_HEADER};
use crate::read::enveloped;
use crate::refusal::Refusal;
use crate::router::Serving;

/// How many unpredictable bytes name one sign-in at the media server.
const DEVICE_BYTES: usize = 16;

pub use attempts::{Attempts, Door, Ticket};
pub use keyed::Keyed;
pub use keyring::{Holding, Keyring};
pub use sessions::Sessions;

/// Where a password is exchanged for a session.
///
/// Named for what it is rather than for its module, because the whole surface is
/// read as one text by the gates that hold the routes to what is written down, and
/// two constants called `PATH` are one a reader resolves to whichever it finds first.
pub const SESSION: &str = "/api/session";

/// The header a refusal for too many wrong answers says how long is left in.
pub const RETRY_AFTER: &str = "Retry-After";

/// What this run knows about who may come in.
///
/// One value rather than three loose ones, because every surface that has to ask
/// needs all of them: the door itself, the guard over every other route, and the
/// stream, which brings its own state and would otherwise have to be handed the
/// pieces separately and could be handed a different set.
#[derive(Default)]
pub struct Admitting {
    /// The sessions this run has opened.
    pub sessions: Sessions,
    /// The wrong answers this run has been given.
    pub attempts: Attempts,
    /// Where the operator's password is kept, where this machine has anywhere to
    /// keep one.
    pub kept: Option<PathBuf>,
    /// Where the household this machine keeps is found. Asked who somebody is when
    /// the machine's own password does not know them.
    pub household: Option<Arc<dyn HouseholdAtHand>>,
    /// The keys other programs hold, read afresh whenever one is presented.
    pub keys: Keyring,
}

/// Where the household is found, at the moment somebody is checked against it.
///
/// Asked on every sign-in and every member's call rather than once at start, the
/// way the operator's credential is read: a stack seeded while this surface was
/// already serving has a household from that moment on.
#[async_trait::async_trait]
pub trait HouseholdAtHand: Send + Sync {
    /// The household as it stands now, or nothing where there is none to ask.
    async fn now(&self) -> Option<Arc<dyn Household>>;

    /// Whether the household vouches for whoever holds this account now, having just
    /// proved its password.
    ///
    /// Asked of the household rather than of the media server, because an invitation
    /// claimed after it ran out signs in like any other account there: whether it was
    /// taken up in time is what this program recorded offering, not anything the server
    /// keeps.
    fn vouches_for(&self, id: &str) -> bool;

    /// Whether `token` claims the invitation standing on account `id` now. A household
    /// that keeps no record of what it offered offers no claim.
    async fn offers_claim(&self, _id: &str, _token: &str) -> bool {
        false
    }

    /// Spend the claim on account `id`'s invitation, where there is a record to spend it
    /// from.
    fn claim_spent(&self, _id: &str) {}
}

/// One household, the same at every asking.
///
/// It keeps no record of what it offered, so it vouches for everybody it signs in.
#[async_trait::async_trait]
impl HouseholdAtHand for Arc<dyn Household> {
    async fn now(&self) -> Option<Arc<dyn Household>> {
        Some(Arc::clone(self))
    }

    fn vouches_for(&self, _: &str) -> bool {
        true
    }
}

/// The household of the stack this surface serves, opened from it at each asking.
#[async_trait::async_trait]
impl HouseholdAtHand for Ctx {
    async fn now(&self) -> Option<Arc<dyn Household>> {
        lemonfiber_core::app::members::household(self).await
    }

    fn vouches_for(&self, id: &str) -> bool {
        lemonfiber_core::app::members::vouched_for(self, id)
    }

    async fn offers_claim(&self, id: &str, token: &str) -> bool {
        lemonfiber_core::app::members::offers_claim(self, id, token).await
    }

    fn claim_spent(&self, id: &str) {
        lemonfiber_core::app::members::claim_spent(self, id);
    }
}

impl Admitting {
    /// The credential as it stands **now**.
    ///
    /// Read at the moment it is asked for rather than held from when the surface
    /// started, and that is the whole mechanism behind two separate promises: a
    /// password changed in another process voids the sessions opened against the old
    /// one, and a password removed while this is serving is gone from here at the
    /// next request rather than at the next restart.
    ///
    /// Absent, unreadable and unreadable-as-a-credential are one answer, which is
    /// the safe direction: nothing here can prove who is knocking, so nobody is let
    /// in on a session.
    #[must_use]
    pub fn credential(&self) -> Option<Credential> {
        self.kept.as_deref().and_then(credential::at)
    }

    /// The credential as it stands now, read on a thread made for blocking.
    ///
    /// What [`Self::credential`] answers, for a caller on the worker that answers
    /// requests: a read from disk there holds up every other request for as long as
    /// the disk takes.
    async fn credential_now(&self) -> Option<Credential> {
        let kept = self.kept.clone()?;
        tokio::task::spawn_blocking(move || credential::at(&kept))
            .await
            .ok()
            .flatten()
    }

    /// The household as it stands now, where there is one to open.
    async fn household_now(&self) -> Option<Arc<dyn Household>> {
        opened(Arc::clone(self.household.as_ref()?)).await
    }

    /// Who a name and a password prove somebody to be, or nothing.
    ///
    /// **Two doors, tried in order, and nothing chooses between them.** The machine's
    /// own password is checked first because it needs no network and no media server;
    /// what it does not match is offered to the household, which is what holds the
    /// accounts everybody else signs in with. A door the attempt's ticket leaves shut
    /// is not tried at all.
    ///
    /// The cost is deliberate and is paid once here: a refusal cannot say which door
    /// was meant. It says the pair was not recognised, because the alternative is
    /// telling somebody which half of their guess to keep working on.
    ///
    /// A household that could not be asked refuses rather than admitting. That is the
    /// safe direction and the honest one — nothing here proved anything, so nobody is
    /// let in on it.
    async fn whoever(
        &self,
        given: &Given,
        ticket: &Ticket,
        random: &dyn Random,
    ) -> Option<(Opened, Door)> {
        // Checked on a thread made for blocking: the hash is built to be slow, and run
        // on the worker that answers requests it would stall every other request for as
        // long as it takes, once per guess.
        if ticket.operator {
            if let Some(held) = self.credential_now().await {
                let offered = given.password.clone();
                let proved =
                    tokio::task::spawn_blocking(move || held.verifies(&offered).then_some(held))
                        .await
                        .ok()
                        .flatten();
                if let Some(held) = proved {
                    return Some((Opened::Operator(held), Door::Operator));
                }
            }
        }
        let door = ticket.member.clone()?;
        let name = given.name.as_deref()?;
        // An account nobody has claimed yet has no password, and the media server
        // lets an empty one sign in to it. That is an invitation still waiting for
        // its person, not a member proving who they are, so an empty password opens
        // nothing here.
        if given.password.is_empty() {
            return None;
        }
        let at = Arc::clone(self.household.as_ref()?);
        let household = opened(Arc::clone(&at)).await?;
        signed_in(at, household.as_ref(), name, &given.password, random)
            .await
            .map(|opened| (opened, door))
    }

    /// Who the secret a request carried proves it to be, or nothing.
    ///
    /// Three secrets answer to the one header and they are not the same claim. The
    /// per-run token is what somebody at this machine's terminal was given; a session
    /// is what somebody who proved the password was given; a key is what the operator
    /// minted for a program. The token and a session are compared over every byte, and
    /// a key is found by its digest over every key kept, so how long any of them takes
    /// says nothing about how much of a guess was right.
    ///
    /// `Nobody` is every refusal of who is asking. There is one of those rather than
    /// several because a caller learning *which* secret was wrong learns which one to
    /// keep guessing at. The two other refusals a key can meet are said apart because
    /// neither is about whether it was right: one is the wait guessing has earned, the
    /// other is a key that crossed a network in the clear.
    pub async fn carried(
        &self,
        headers: &HeaderMap,
        token: &Token,
        arrived: Option<Arrived>,
        now: SystemTime,
    ) -> Knocking {
        let offered = headers
            .get(TOKEN_HEADER)
            .and_then(|value| value.to_str().ok());
        if token.carried_by(offered) {
            return Knocking::Known(Caller::Machine);
        }
        if let Some(key) = offered.filter(|offered| lemonfiber_core::keys::shaped(offered)) {
            return self.keyed(key, arrived, now).await;
        }
        // The credential is read only for a session that was opened against one, so a
        // request carrying nothing, or a secret this run never handed out, costs no
        // read at all. Its absence is not *nobody is admitted*: a machine keeping none
        // still has member sessions to answer for.
        let opened = self.sessions.opened_for(offered, now).await;
        let against = match &opened {
            Some(Opened::Operator(_)) => self.credential_now().await,
            Some(Opened::Member(_)) | None => None,
        };
        match opened.and_then(|opened| sessions::still(opened, against.as_ref())) {
            Some(Opened::Operator(_)) => Knocking::Known(Caller::Operator),
            // Re-read on every call, the way the operator's credential above it is.
            // A session is a claim about an identity and only the media server can
            // say whether that identity is still one, so an account removed there
            // reaches the person holding it at their next call rather than at their
            // next sign-in.
            Some(Opened::Member(signed)) => self.still_standing(signed).await,
            None => Knocking::Nobody,
        }
    }

    /// Whether the household still holds this member, as an answer to knock with.
    ///
    /// **Three answers, because there are three facts.** Still here is that member.
    /// Gone is nobody, and is the whole reason this is asked on every call rather
    /// than at sign-in. Could-not-ask is neither:
    /// collapsing it into *gone* would sign a household out for the length of a
    /// media-server reboot and tell them their account had been removed, which is
    /// the same mistake the sign-in door is built to avoid one floor down.
    async fn still_standing(&self, signed: Signed) -> Knocking {
        let Some(household) = self.household_now().await else {
            // With no household to open there is nobody to ask whether this member
            // still stands, so the session is one this run cannot vouch for.
            return Knocking::Unconfirmed;
        };
        match household.standing(&signed).await {
            Ok(true) => Knocking::Known(Caller::Member(signed.id)),
            Ok(false) => Knocking::Nobody,
            Err(_) => Knocking::Unconfirmed,
        }
    }
}

/// The household `at` holds now, opened on a thread made for blocking.
///
/// Opening one reads the stack's manifest and its recorded password from disk, and the
/// worker that answers requests is no place to wait on a disk.
async fn opened(at: Arc<dyn HouseholdAtHand>) -> Option<Arc<dyn Household>> {
    tokio::task::spawn_blocking(move || tokio::runtime::Handle::current().block_on(at.now()))
        .await
        .ok()
        .flatten()
}

/// What the guard learned when somebody knocked.
///
/// Three answers rather than two, and the third is the whole point of the type.
/// **Nobody** is a secret this run does not admit, or an identity the household no
/// longer holds — both are settled facts and both refuse. **Unconfirmed** is neither
/// of those: the question was asked and could not be answered, so nobody has been
/// identified, which is a different thing from having identified nobody. A guard
/// that answered both with the same silence would tell a household their accounts
/// were gone on the day their media server was restarting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Knocking {
    /// Proved to be this caller.
    Known(Caller),
    /// Proved nothing this run admits, or proved an identity that no longer stands.
    Nobody,
    /// Could not be established, which is not the same as nobody.
    Unconfirmed,
    /// A key arrived while the wrong answers so far have earned a wait, and was not
    /// looked at. How long is left.
    Held(Duration),
    /// A key arrived from another machine over a connection its pin does not verify,
    /// and was not looked at.
    Exposed,
}

/// Who a request proved itself to be.
///
/// Who, rather than only whether: a surface that must refuse one person what it
/// offers another cannot be built on an indistinguishable *yes*. It would have to
/// decide for itself which person is looking, and a control withheld on that basis
/// is withheld by whoever drew the screen.
///
/// So the question the guard answers is the subject rather than the verdict. What
/// each caller may then do is decided where it is known — never here, and never by
/// a client reading this and drawing its own conclusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    /// Somebody at this machine's terminal, carrying the token printed there.
    Machine,
    /// Somebody who proved the password this machine keeps.
    Operator,
    /// Somebody the media server holds an account for, by the id it files them
    /// under. What they may then do is the core's answer and is decided where it is
    /// known — never from this, and never by a client reading it.
    Member(String),
    /// A program holding a key the operator minted for it.
    Key(Keyed),
}

impl Caller {
    /// The household member this caller acts as, where it acts as one: a member's own
    /// session, or a key scoped to them.
    #[must_use]
    pub fn member(&self) -> Option<&str> {
        match self {
            Self::Member(id) => Some(id),
            Self::Key(keyed) => keyed.scope.member(),
            Self::Machine | Self::Operator => None,
        }
    }
}

/// Who is asking, taken from what the guard admitted.
///
/// One impl rather than a branch per handler: the guard puts the subject on the
/// request, and a handler that needs it says so in its signature. Absent means the
/// request reached a handler without the guard having named anybody, which past the
/// guard happens only at the one door that opens without a secret — so it is
/// answered as what it is, a request carrying nothing this run admits, rather than
/// served as though somebody had proved something.
impl<S: Send + Sync> FromRequestParts<S> for Caller {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or_else(|| Refusal::NotAdmitted.answered())
    }
}

/// What a caller offers at the door.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(rename = "SignIn")]
struct Given {
    /// Who they say they are, where they say so. Absent from an operator signing in
    /// with the machine's own password, which is nobody's name.
    #[serde(default)]
    name: Option<String>,
    /// What was typed: at a claim, the password they chose.
    password: String,
    /// The claim token an invitation's join link carries, where this is a claim.
    #[serde(default)]
    claim: Option<String>,
}

/// The body signing in takes, with the route it is sent to, described from the type
/// the route reads it into.
pub(crate) fn body() -> (&'static str, schemars::Schema) {
    (SESSION, schemars::schema_for!(Given))
}

/// The one route.
///
/// Merged with the rest and under the same layer, so an endpoint added beside it is
/// guarded by having been added. What the layer does differently for this one path
/// is written where the layer is.
pub fn routes() -> Router<Serving> {
    Router::new().route(SESSION, post(opening))
}

/// Whether a request says it came from where this server is listening.
#[must_use]
pub fn here(headers: &HeaderMap, at: &Binding) -> bool {
    let said = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    host_is_here(said(header::HOST.as_str()), at)
        && origin_is_here(said(header::ORIGIN.as_str()), at)
}

/// Exchange a password for a session.
async fn opening(
    State(serving): State<Serving>,
    connected: Option<Extension<ConnectInfo<SocketAddr>>>,
    given: Result<Json<Given>, JsonRejection>,
) -> Response {
    let Ok(Json(given)) = given else {
        return Refusal::NotAPassword.answered();
    };
    if given.claim.is_some() && given.password.chars().count() < credential::LEAST {
        return Refusal::ShortChoice.answered();
    }
    let now = serving.ctx.seams.clock.now();
    let ticket = match serving
        .admitting
        .attempts
        .taken(peer(connected), given.name.as_deref(), now)
        .await
    {
        Ok(ticket) => ticket,
        Err(left) => return waiting(left.as_secs().max(1)),
    };
    let random = serving.ctx.seams.random.as_ref();
    let admitted = match given.claim.as_deref() {
        Some(token) => {
            serving
                .admitting
                .claimed(&given, token, &ticket, random)
                .await
        }
        None => serving
            .admitting
            .whoever(&given, &ticket, random)
            .await
            .ok_or(Refusal::NotThePassword),
    };
    let (who, door) = match admitted {
        Ok(admitted) => admitted,
        Err(refusal) => return refusal.answered(),
    };
    serving.admitting.attempts.right(&ticket, door, now).await;
    let opened = serving
        .admitting
        .sessions
        .opened(serving.ctx.seams.random.as_ref(), now, who)
        .await;
    enveloped(
        StatusCode::OK,
        opened.and_then(|opened| Envelope::new(kind::ADMISSION, opened).to_json()),
    )
}

/// The address a request came from, or nothing for a surface answered without a
/// socket — a test, driving the router directly.
fn peer(connected: Option<Extension<ConnectInfo<SocketAddr>>>) -> Option<IpAddr> {
    connected.map(|Extension(ConnectInfo(at))| at.ip())
}

/// Too many wrong answers, and how long is left.
///
/// The wait is said in the header a client already knows to read and in the sentence
/// a person reads, because both of them are here: the page shows one and the client
/// behind it waits on the other.
pub(crate) fn waiting(seconds: u64) -> Response {
    let mut response = Refusal::TooManyAttempts.saying(format!(
        "Too many wrong passwords and keys. Try again in {seconds} seconds."
    ));
    response
        .headers_mut()
        .insert(RETRY_AFTER, axum::http::HeaderValue::from(seconds));
    response
}
