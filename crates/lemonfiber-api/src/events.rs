//! The stream a browser leaves open, and what it hears on it.
//!
//! Everything else here answers a question and closes. This one stays, which
//! makes two things a client cannot otherwise know its own concern. Whether the
//! server is still there, which silence alone does not say — so silence is
//! broken on a beat. And where a client got to before it was cut off, which is
//! what an id on every event and a backlog behind them answer.
//!
//! What the stream will not do is make a gap look like continuity. A figure
//! gathered before a client was disconnected is not handed back to it as though
//! it were current; only a gather made since is.

pub mod backlog;
pub mod dashboard;
pub mod live;
pub mod saying;
pub mod stepping;
pub mod wire;

use std::convert::Infallible;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Response, StatusCode};
use axum::routing::get;
use axum::Router;

use lemonfiber_core::ports::time::Clock;

use crate::admission::Knocking;
use crate::guard::{Binding, Token};
use crate::serve::{admitted, carrying, refused, Refusal, STREAM};

use self::live::{Listening, Live};

/// Where the stream is served.
pub const PATH: &str = "/api/events";

/// The header a client returning to the stream says where it got to in.
pub const LAST_EVENT_ID: &str = "Last-Event-ID";

/// What answering the stream takes: who may listen, and what they hear.
pub struct Streaming {
    /// This run's token, which every request must carry.
    ///
    /// The same one the rest of the surface is guarded by, shared rather than
    /// minted again: two secrets for one run would be a run a client could be
    /// admitted to half of.
    pub token: Arc<Token>,
    /// Where this server is listening, as much of it as every request must name.
    pub bound: Binding,
    /// Who this run has let in, so a listener holding a session is heard too.
    ///
    /// The same register the rest of the surface reads. Two would be a run somebody
    /// could be admitted to half of, which is the reason this shares the token
    /// rather than minting a second one.
    pub admitting: Arc<crate::admission::Admitting>,
    /// The one gather, and everyone already listening to it.
    pub live: Arc<Live>,
    /// The clock the rest of the surface reads.
    ///
    /// Carried for the reason the token and the register are. This guard decides
    /// whether a session has ended, and the guard above every other route decides
    /// the same thing from the context's clock — two readings of *when it is* can
    /// disagree at the moment a session expires, and the route that disagreed would
    /// be the one nobody tested. Held rather than asked of the platform here.
    pub clock: Arc<dyn Clock>,
}

/// The event stream's route, for the surface to merge with the rest.
///
/// It carries its own state rather than the one the other endpoints share,
/// because what it needs is what is being said and not the world a command runs
/// against. So it arrives already stated and merges at the top of the tree.
pub fn routes(streaming: Arc<Streaming>) -> Router {
    Router::new().route(PATH, get(stream)).with_state(streaming)
}

/// Answer a request to listen.
///
/// A stream held open is a request that has not finished, not a request that was
/// never made, so it meets the same admission every other request does. Checked
/// here rather than left to whoever assembles the tree: this route brings its own
/// state and can therefore be merged outside the layer that guards the rest,
/// which is an assembly mistake that would otherwise leave it open.
pub async fn stream(State(streaming): State<Arc<Streaming>>, headers: HeaderMap) -> Response<Body> {
    let now = streaming.clock.now();
    let knocking = streaming
        .admitting
        .carried(&headers, &streaming.token, now)
        .await;
    if matches!(knocking, Knocking::Unconfirmed) {
        return refused(Refusal::Unconfirmed);
    }
    // The stream carries the operator's whole view — the dashboard, every log line
    // the operator follows, what setup is doing — and nothing on it is narrowed to a
    // member, so a member is refused it rather than handed the operator's copy.
    if let Knocking::Known(caller) = &knocking {
        if let Some(refusal) = crate::serve::operator_only(caller) {
            return refusal;
        }
    }
    if let Err(refusal) = admitted(
        matches!(knocking, Knocking::Known(_)),
        &headers,
        streaming.bound,
    ) {
        return refused(refusal);
    }
    let seen = headers
        .get(LAST_EVENT_ID)
        .and_then(|seen| seen.to_str().ok());
    let listening = streaming.live.listening(seen).await;
    // Asked for after the client is listening, so the gather it prompts is one
    // this client hears — which is what replaces whatever it still holds.
    streaming.live.nudge();
    held(listening)
}

/// The stream, as a response a client holds open.
#[must_use]
pub fn held(listening: Listening) -> Response<Body> {
    let talking = futures_util::stream::unfold(listening, |mut listening| async move {
        let said = listening.next().await?;
        Some((Ok::<String, Infallible>(said), listening))
    });
    carrying(StatusCode::OK, STREAM, Body::from_stream(talking))
}
