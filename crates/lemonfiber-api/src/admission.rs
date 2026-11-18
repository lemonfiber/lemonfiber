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

mod caller;
pub mod coded;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap};
use axum::response::Response;
use axum::routing::post;
use axum::{Extension, Json, Router};
use lemonfiber_core::app::admitting::{Answer, Asked, Carried, Offered};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::error::Problem;
use lemonfiber_core::ports::asking::Asking;
use serde::Deserialize;

use crate::guard::{host_is_here, origin_is_here, Arrived, Binding, Token, TOKEN_HEADER};
use crate::read::carried_out;
use crate::refusal::Refusal;
use crate::router::Serving;
use coded::CODE;

pub use caller::{Caller, Keyed, Knocking};

/// Where a password is exchanged for a session.
///
/// Named for what it is rather than for its module, because the whole surface is
/// read as one text by the gates that hold the routes to what is written down, and
/// two constants called `PATH` are one a reader resolves to whichever it finds first.
pub const SESSION: &str = "/api/session";

/// The header a refusal for too many wrong answers says how long is left in.
pub const RETRY_AFTER: &str = "Retry-After";

/// Who the secret a request carried proves it to be: the per-run token is checked here,
/// and a session or a key through the dispatcher.
pub async fn knocking(
    ctx: &Ctx,
    headers: &HeaderMap,
    token: &Token,
    arrived: Option<Arrived>,
) -> Knocking {
    let offered = headers
        .get(TOKEN_HEADER)
        .and_then(|value| value.to_str().ok());
    if token.carried_by(offered) {
        return Knocking::Known(Caller::Machine);
    }
    let carried = Carried {
        offered: offered.map(str::to_owned),
        arrived,
    };
    knocked(dispatch(Command::Admission(Asked::Caller(carried)), ctx).await)
}

/// Who a caller's check answered, and nobody where it answered anything else.
fn knocked(answered: Result<Outcome, Box<Problem>>) -> Knocking {
    match answered {
        Ok(Outcome::Admitting(Answer::Caller(calling))) => calling.into(),
        _ => Knocking::Nobody,
    }
}

/// What a request offered as its password, the answer it gives when the core asks.
struct Typed(String);

impl Asking for Typed {
    fn present(&self) -> bool {
        true
    }

    fn ask(&self, _question: &str) -> String {
        String::new()
    }

    fn secret(&self, _question: &str) -> String {
        self.0.clone()
    }
}

/// `ctx`, answering with `password` when the core asks the asker for theirs.
pub(crate) fn typed(ctx: &Ctx, password: String) -> Ctx {
    ctx.clone().with_asking(Arc::new(Typed(password)))
}

/// What a caller offers at the door.
#[derive(Deserialize, schemars::JsonSchema)]
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

/// The two routes: a password exchanged for a session, and a code signed in by.
///
/// Merged with the rest and under the same layer, so an endpoint added beside it is
/// guarded by having been added. What the layer does differently for this one path
/// is written where the layer is.
pub fn routes() -> Router<Serving> {
    Router::new()
        .route(SESSION, post(opening))
        .route(CODE, post(coded::coded))
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
    let offered = Offered {
        name: given.name,
        claim: given.claim,
        peer: peer(connected),
    };
    let ctx = typed(&serving.ctx, given.password);
    carried_out(&ctx, Command::Admission(Asked::Password(offered))).await
}

/// The address a request came from, or nothing for a surface answered without a
/// socket — a test, driving the router directly.
fn peer(connected: Option<Extension<ConnectInfo<SocketAddr>>>) -> Option<IpAddr> {
    connected.map(|Extension(ConnectInfo(at))| at.ip())
}

#[cfg(test)]
mod tests;
