//! Registering, listing and revoking the keys a phone proves the operator with, and
//! the challenges it signs.
//!
//! **The operator's alone.** A member session and a key of any scope are refused at all
//! four routes: a device key proves the operator, and nobody else has anything to prove
//! with one.
//!
//! **Registering asks for the password in the same request**, over the connection a key
//! may cross, as minting a key does: the registration is what lets a phone stand in for
//! the password from then on.
//!
//! **A challenge belongs to the token that asked for it.** It is good for one mint from
//! the session or run that holds that token, with that device key, for one minute.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};

use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Extension, Json, Router};
use lemonfiber_core::app::Command;
use lemonfiber_core::device_keys::run::Asked;
use lemonfiber_core::device_keys::Attestation;
use serde::Deserialize;

use crate::admission::Caller;
use crate::guard::{Arrived, Presented};
use crate::read::carried_out;
use crate::refusal::Refusal;
use crate::router::Serving;

/// Where device keys are listed and registered.
pub const DEVICE_KEYS: &str = "/api/device-keys";

/// Where one device key is revoked, by its name.
pub const A_DEVICE_KEY: &str = "/api/device-keys/{name}";

/// Where a challenge for one device key is asked for.
pub const CHALLENGE: &str = "/api/device-keys/{name}/challenge";

/// What a registration is asked with.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DeviceKeyRegistering")]
struct Registering {
    /// What to call it: lower-case letters, digits, dots, dashes and underscores.
    name: String,
    /// The public half of a P-256 key pair, as its `SubjectPublicKeyInfo` in base64url,
    /// its point uncompressed.
    public_key: String,
    /// The platform's attestation of where the key lives.
    #[serde(default)]
    attestation: Option<Attestation>,
    /// The operator's password, given again.
    password: String,
}

/// What a registration asked for, the password and the key withheld.
impl std::fmt::Debug for Registering {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Registering")
            .field("name", &self.name)
            .field("attestation", &self.attestation)
            .finish_non_exhaustive()
    }
}

/// The body a registration takes, with the route it is sent to.
pub(crate) fn body() -> (&'static str, schemars::Schema) {
    (DEVICE_KEYS, schemars::schema_for!(Registering))
}

/// The four routes device keys are kept through.
pub fn routes() -> Router<Serving> {
    Router::new()
        .route(DEVICE_KEYS, get(listed).post(registered))
        .route(A_DEVICE_KEY, delete(revoked))
        .route(CHALLENGE, post(challenged))
}

/// Every device key.
async fn listed(State(serving): State<Serving>, caller: Caller) -> Response {
    match operator(&caller) {
        Ok(()) => carried_out(&serving.ctx, Command::DeviceKeys(Asked::List)).await,
        Err(refused) => *refused,
    }
}

/// A device key registered, once the password proves the operator.
async fn registered(
    State(serving): State<Serving>,
    caller: Caller,
    arrived: Option<Extension<Arrived>>,
    given: Result<Json<Registering>, JsonRejection>,
) -> Response {
    if let Err(refused) = operator(&caller) {
        return *refused;
    }
    let Some(Extension(arrived)) = arrived.filter(|Extension(arrived)| arrived.may_carry_a_key())
    else {
        return Refusal::KeyInTheClear.answered();
    };
    let Ok(Json(given)) = given else {
        return Refusal::NotAKeyRequest.answered();
    };
    let asked = Asked::Register {
        name: given.name,
        public_key: given.public_key,
        attestation: given.attestation,
        password: given.password,
        from: Some(arrived.from),
    };
    carried_out(&serving.ctx, Command::DeviceKeys(asked)).await
}

/// A device key revoked, by its name, and every challenge standing for it spent.
async fn revoked(
    State(serving): State<Serving>,
    caller: Caller,
    Path(name): Path<String>,
) -> Response {
    if let Err(refused) = operator(&caller) {
        return *refused;
    }
    carried_out(&serving.ctx, Command::DeviceKeys(Asked::Revoke { name })).await
}

/// A challenge for one registered device key, bound to the token that asked.
async fn challenged(
    State(serving): State<Serving>,
    caller: Caller,
    Extension(Presented(token)): Extension<Presented>,
    Path(name): Path<String>,
) -> Response {
    if let Err(refused) = operator(&caller) {
        return *refused;
    }
    let asked = Asked::Challenge {
        name,
        session: token,
    };
    carried_out(&serving.ctx, Command::DeviceKeys(asked)).await
}

/// Nothing where this caller is the operator, by a session or by this run's own token,
/// and the refusal it is answered with otherwise.
pub(crate) fn operator(caller: &Caller) -> Result<(), Box<Response>> {
    match caller {
        Caller::Operator | Caller::Machine => Ok(()),
        Caller::Member(_) => Err(Box::new(crate::refusal::not_yours(Some(
            "Device keys are the operator's, and only theirs.",
        )))),
        Caller::Key(_) => Err(Box::new(Refusal::NotForAKey.saying(
            "A key may not register, list or revoke device keys, whatever its scope.",
        ))),
    }
}

#[cfg(test)]
mod tests;
