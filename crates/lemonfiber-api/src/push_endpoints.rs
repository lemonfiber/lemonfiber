//! Registering, listing and removing the phones that asked to be woken.
//!
//! **The operator's alone.** A member session and a key of any scope are refused at all
//! three routes: an endpoint is told when the stack speaks, whichever member's business it
//! is about.
//!
//! **A registration carries a secret**, so it takes the connection a key may cross.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::response::Response;
use axum::routing::{delete, get};
use axum::{Extension, Json, Router};
use lemonfiber_core::app::Command;
use lemonfiber_core::push::run::Asked;
use serde::Deserialize;

use crate::admission::Caller;
use crate::guard::Arrived;
use crate::read::carried_out;
use crate::refusal::Refusal;
use crate::router::Serving;

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

/// The three routes push endpoints are kept through.
pub fn routes() -> Router<Serving> {
    Router::new()
        .route(PUSH_ENDPOINTS, get(listed).post(registered))
        .route(A_PUSH_ENDPOINT, delete(removed))
}

/// Every push endpoint.
async fn listed(State(serving): State<Serving>, caller: Caller) -> Response {
    match operator(&caller) {
        Ok(()) => carried_out(&serving.ctx, Command::PushEndpoints(Asked::List)).await,
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
    if let Err(refused) = operator(&caller) {
        return *refused;
    }
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
    };
    let answer = carried_out(&serving.ctx, Command::PushEndpoints(asked)).await;
    serving.live.roused();
    answer
}

/// A push endpoint removed, by its name.
async fn removed(
    State(serving): State<Serving>,
    caller: Caller,
    Path(name): Path<String>,
) -> Response {
    match operator(&caller) {
        Ok(()) => carried_out(&serving.ctx, Command::PushEndpoints(Asked::Remove { name })).await,
        Err(refused) => *refused,
    }
}

/// Nothing where this caller is the operator, by a session or by this run's own token,
/// and the refusal it is answered with otherwise.
fn operator(caller: &Caller) -> Result<(), Box<Response>> {
    match caller {
        Caller::Operator | Caller::Machine => Ok(()),
        Caller::Member(_) => Err(Box::new(
            Refusal::NotYours.saying("Push endpoints are the operator's, and only theirs."),
        )),
        Caller::Key(_) => Err(Box::new(Refusal::NotForAKey.saying(
            "A key may not register, list or remove push endpoints, whatever its scope.",
        ))),
    }
}

#[cfg(test)]
mod tests;
