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
//! A wrong password answers `401` rather than the `403` every other refusal
//! answers with, and the difference is the reason to have two: `403` means *nothing
//! you could send would help*, which is true of a missing token and false of a
//! wrong password. A client that cannot tell them apart cannot know whether
//! offering a login is worth anything.

pub mod attempts;
pub mod sessions;

use std::path::PathBuf;
use std::time::SystemTime;

use axum::body::Body;
use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::post;
use axum::{Json, Router};
use lemonfiber_core::admission::{credential, Credential};
use lemonfiber_core::model::{kind, Envelope};
use serde::Deserialize;

use crate::guard::{host_is_here, origin_is_here, Binding, Token, TOKEN_HEADER};
use crate::read::enveloped;
use crate::router::Serving;
use crate::serve::{carrying, SENTENCE};

pub use attempts::Attempts;
pub use sessions::Sessions;

/// Where a password is exchanged for a session.
///
/// Named for what it is rather than for its module, because the whole surface is
/// read as one text by the gates that hold the routes to what is written down, and
/// two constants called `PATH` are one a reader resolves to whichever it finds first.
pub const SESSION: &str = "/api/session";

/// The header a refusal for too many wrong answers says how long is left in.
pub const RETRY_AFTER: &str = "Retry-After";

/// What is said to a request whose body is not a password.
const NOT_A_PASSWORD: &str = "The body of this request is not a password.";

/// What is said to a wrong answer, and to a right one where nothing is set.
///
/// One sentence for both, because they are the same fact to whoever is knocking:
/// what they sent did not open the door. Saying which of the two it was would tell
/// somebody guessing whether there is anything here to guess at.
const NOT_THE_PASSWORD: &str = "That is not the password for this machine.";

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

    /// Whether the secret a request carried is one this run admits.
    ///
    /// Two secrets answer to the one header. The per-run token is what the operator
    /// at this machine was given; a session is what somebody who proved the password
    /// was given. Both are compared over every byte, so how long either takes says
    /// nothing about how much of a guess was right.
    pub async fn carried(&self, headers: &HeaderMap, token: &Token, now: SystemTime) -> bool {
        let offered = headers
            .get(TOKEN_HEADER)
            .and_then(|value| value.to_str().ok());
        if token.carried_by(offered) {
            return true;
        }
        match self.credential() {
            Some(held) => self.sessions.holds(offered, now, &held).await,
            None => false,
        }
    }
}

/// The password offered, as a caller sends it.
#[derive(Debug, Deserialize)]
struct Given {
    /// What was typed.
    password: String,
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
pub fn here(headers: &HeaderMap, at: Binding) -> bool {
    let said = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    host_is_here(said(header::HOST.as_str()), at)
        && origin_is_here(said(header::ORIGIN.as_str()), at)
}

/// Exchange a password for a session.
async fn opening(
    State(serving): State<Serving>,
    given: Result<Json<Given>, JsonRejection>,
) -> Response {
    let Ok(Json(given)) = given else {
        return said(StatusCode::BAD_REQUEST, NOT_A_PASSWORD);
    };
    let now = serving.ctx.clock.now();
    if let Some(left) = serving.admitting.attempts.waiting(now).await {
        return waiting(left.as_secs().max(1));
    }
    let held = serving.admitting.credential();
    let Some(held) = held.filter(|held| held.verifies(&given.password)) else {
        serving.admitting.attempts.wrong(now).await;
        return said(StatusCode::UNAUTHORIZED, NOT_THE_PASSWORD);
    };
    serving.admitting.attempts.right().await;
    let opened = serving
        .admitting
        .sessions
        .opened(serving.ctx.random.as_ref(), now, &held)
        .await;
    enveloped(
        StatusCode::OK,
        opened.and_then(|opened| Envelope::new(kind::ADMISSION, opened).to_json()),
    )
}

/// A sentence, at the status it is said under.
fn said(status: StatusCode, sentence: &'static str) -> Response {
    carrying(status, SENTENCE, Body::from(sentence))
}

/// Too many wrong answers, and how long is left.
///
/// The wait is said in the header a client already knows to read and in the sentence
/// a person reads, because both of them are here: the page shows one and the client
/// behind it waits on the other.
fn waiting(seconds: u64) -> Response {
    let mut response = carrying(
        StatusCode::TOO_MANY_REQUESTS,
        SENTENCE,
        Body::from(format!(
            "Too many wrong passwords. Try again in {seconds} seconds."
        )),
    );
    response
        .headers_mut()
        .insert(RETRY_AFTER, axum::http::HeaderValue::from(seconds));
    response
}
