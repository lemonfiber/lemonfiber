//! What revoking an integration key, removing a phone's hardware key and removing a wake
//! address have in common: a body asking for a rehearsal, and who may ask for each.
//!
//! Each rehearses as an action does. Sent without a body, or with `dry_run` false, each
//! removes what it names; sent `{ "dry_run": true }`, each removes nothing and answers with
//! what its command's rehearsal renders. Anything else in the body is refused before
//! anything is looked up, so a mistyped field never revokes a key it meant to rehearse.

use std::sync::Arc;

use axum::response::Response;
use lemonfiber_core::app::Ctx;
use schemars::Schema;
use serde::Deserialize;

use crate::admission::Caller;
use crate::device_keys::A_DEVICE_KEY;
use crate::keys::A_KEY;
use crate::push_endpoints::A_PUSH_ENDPOINT;
use crate::refusal::Refusal;

/// The body a revocation takes.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "Revoking")]
struct Revoking {
    /// Whether to answer with what revoking would come to, and revoke nothing.
    #[serde(default)]
    dry_run: bool,
}

/// The run a revocation is carried out in: the surface's own, or a rehearsal of it where
/// `body` asks for one.
///
/// # Errors
///
/// The refusal a body other than `dry_run` alone is answered with.
pub(crate) fn carried_in(ctx: &Arc<Ctx>, body: &[u8]) -> Result<Arc<Ctx>, Box<Response>> {
    if body.is_empty() {
        return Ok(Arc::clone(ctx));
    }
    serde_json::from_slice::<Revoking>(body)
        .map(|given| crate::actions::asked_of(ctx, given.dry_run))
        .map_err(|_| Box::new(Refusal::NotARevocation.answered()))
}

/// The body each revocation takes, with the route it is sent to.
pub(crate) fn bodies() -> [(&'static str, Schema); 3] {
    [A_KEY, A_DEVICE_KEY, A_PUSH_ENDPOINT].map(|route| (route, schemars::schema_for!(Revoking)))
}

/// Each revocation, by the path it is served at, with whether `caller` may ask for it as
/// its route decides.
pub(crate) fn permitted(caller: &Caller) -> [(&'static str, bool); 3] {
    [
        (A_KEY, crate::keys::keeper(caller).is_ok()),
        (A_DEVICE_KEY, crate::device_keys::operator(caller).is_ok()),
        (
            A_PUSH_ENDPOINT,
            crate::push_endpoints::owner(caller).is_ok(),
        ),
    ]
}

#[cfg(test)]
mod tests;
