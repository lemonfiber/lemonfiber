//! Where a curator or the media server tells the core what happened.
//!
//! **Open without a session, and taken only with a key.** A service holds no session and no
//! key to this surface: it holds the one key the core minted for it when it was wired, and
//! presents that as its bearer token. Anything else is refused before the event is read,
//! and a key is taken only over a connection that may carry one.

use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use lemonfiber_core::app::{dispatch, Command, Surfacing};
use lemonfiber_core::notify::events::{Event, Happened, Telling};

use crate::guard::Arrived;
use crate::read::went_wrong;
use crate::refusal::Refusal;
use crate::router::Serving;
use crate::routing::{Serves, Taking};

/// Where a service tells the core what happened.
pub const EVENTS: &str = "/api/notify/events";

/// The scheme a service presents its key under.
const BEARER: &str = "Bearer ";

/// The body this route reads, as the contract describes it.
pub(crate) fn body() -> (&'static str, schemars::Schema) {
    (EVENTS, schemars::schema_for!(Event))
}

/// The route, answering as the command it reaches does.
#[must_use]
pub fn routes() -> Serves<Serving> {
    let telling = Telling {
        presented: String::new(),
        event: Event {
            event: Happened::Imported,
            reference: None,
            name: String::new(),
            reason: None,
            cleared: false,
        },
        from: None,
    };
    let answered = Command::Surface(Surfacing::Told(telling)).answered_under();
    Serves::new().post(EVENTS, told, Taking::Body, answered)
}

/// One event, taken where the key it came with is one the core handed a service.
async fn told(
    State(serving): State<Serving>,
    arrived: Option<Extension<Arrived>>,
    headers: HeaderMap,
    given: Result<Json<Event>, JsonRejection>,
) -> Response {
    let Some(Extension(arrived)) = arrived.filter(|Extension(arrived)| arrived.may_carry_a_key())
    else {
        return Refusal::KeyInTheClear.answered();
    };
    let presented = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix(BEARER))
        .unwrap_or_default();
    let Ok(Json(event)) = given else {
        return Refusal::NotAKeyRequest.answered();
    };
    let telling = Telling {
        presented: presented.to_owned(),
        event,
        from: Some(arrived.from),
    };
    match dispatch(Command::Surface(Surfacing::Told(telling)), &serving.ctx).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(problem) => went_wrong(&problem),
    }
}
