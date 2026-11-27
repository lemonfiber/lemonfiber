//! Registering, listing and removing the phones that asked to be woken.
//!
//! **The operator's, and each member's own.** The operator's phones are told when the
//! stack speaks; a member session registers, lists and removes only its own phone, which is
//! told only that something they asked for is ready. A key of any scope is refused at all
//! three routes.
//!
//! **A registration carries a secret**, so it takes the connection a key may cross.

use axum::body::Bytes;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::response::Response;
use axum::{Extension, Json};
use lemonfiber_core::app::Command;
use lemonfiber_core::push::run::Asked;
use serde::Deserialize;

use crate::admission::Caller;
use crate::guard::Arrived;
use crate::read::carried_out;
use crate::refusal::Refusal;
use crate::router::Serving;
use crate::routing::{Serves, Taking};

/// Where push endpoints are listed and registered.
pub const PUSH_ENDPOINTS: &str = "/api/push-endpoints";

/// Where one push endpoint is removed, by its name.
pub const A_PUSH_ENDPOINT: &str = "/api/push-endpoints/{name}";

/// What a registration is asked with.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PushEndpointRegistering")]
struct Registering {
    /// What to call it: lower-case letters, digits, dots, dashes and underscores.
    name: String,
    /// The `https` address the phone's `UnifiedPush` distributor gave.
    endpoint: String,
    /// The phone's `WebPush` public key, in base64url.
    p256dh: String,
    /// The phone's `WebPush` secret, in base64url.
    auth: String,
}

/// What a registration asked for, its keys and its address withheld.
impl std::fmt::Debug for Registering {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Registering")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// The body a registration takes, with the route it is sent to.
pub(crate) fn body() -> (&'static str, schemars::Schema) {
    (PUSH_ENDPOINTS, schemars::schema_for!(Registering))
}

/// The three routes push endpoints are kept through, each answering as the command it
/// reaches does.
#[must_use]
pub fn routes() -> Serves<Serving> {
    let registering = Asked::Register {
        name: String::new(),
        endpoint: String::new(),
        p256dh: String::new(),
        auth: String::new(),
        owner: None,
    };
    let removing = Asked::Remove {
        name: String::new(),
        owner: None,
    };
    let answered = |asked| Command::PushEndpoints(asked).answered_under();
    Serves::new()
        .get(
            PUSH_ENDPOINTS,
            listed,
            answered(Asked::List { owner: None }),
        )
        .post(
            PUSH_ENDPOINTS,
            registered,
            Taking::Body,
            answered(registering),
        )
        .delete(A_PUSH_ENDPOINT, removed, Taking::Body, answered(removing))
}

/// Every push endpoint.
async fn listed(State(serving): State<Serving>, caller: Caller) -> Response {
    match owner(&caller) {
        Ok(owner) => carried_out(&serving.ctx, Command::PushEndpoints(Asked::List { owner })).await,
        Err(refused) => *refused,
    }
}

/// A push endpoint registered, and the stack read from now on whoever is listening.
async fn registered(
    State(serving): State<Serving>,
    caller: Caller,
    arrived: Option<Extension<Arrived>>,
    given: Result<Json<Registering>, JsonRejection>,
) -> Response {
    let owner = match owner(&caller) {
        Ok(owner) => owner,
        Err(refused) => return *refused,
    };
    if !arrived.is_some_and(|Extension(arrived)| arrived.may_carry_a_key()) {
        return Refusal::KeyInTheClear.answered();
    }
    let Ok(Json(given)) = given else {
        return Refusal::NotAKeyRequest.answered();
    };
    let asked = Asked::Register {
        name: given.name,
        endpoint: given.endpoint,
        p256dh: given.p256dh,
        auth: given.auth,
        owner,
    };
    let answer = carried_out(&serving.ctx, Command::PushEndpoints(asked)).await;
    serving.live.roused();
    answer
}

/// A push endpoint removed, by its name, or that removal rehearsed where the body asks
/// for it.
async fn removed(
    State(serving): State<Serving>,
    caller: Caller,
    Path(name): Path<String>,
    body: Bytes,
) -> Response {
    let asked = owner(&caller)
        .and_then(|owner| crate::revoking::carried_in(&serving.ctx, &body).map(|ctx| (ctx, owner)));
    match asked {
        Ok((ctx, owner)) => {
            carried_out(&ctx, Command::PushEndpoints(Asked::Remove { name, owner })).await
        }
        Err(refused) => *refused,
    }
}

/// Whose phones this caller reaches: every one for the operator, by a session or by this
/// run's own token, a member's own for a member session, and the refusal a key is answered
/// with.
pub(crate) fn owner(caller: &Caller) -> Result<Option<String>, Box<Response>> {
    match caller {
        Caller::Operator | Caller::Machine => Ok(None),
        Caller::Member(id) => Ok(Some(id.clone())),
        Caller::Key(_) => Err(Box::new(Refusal::NotForAKey.saying(
            "A key may not register, list or remove push endpoints, whatever its scope.",
        ))),
    }
}

#[cfg(test)]
mod tests;
