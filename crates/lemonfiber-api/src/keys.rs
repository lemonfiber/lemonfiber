//! Minting, listing and revoking keys from a browser or the companion.
//!
//! The one credential write this surface takes, and it is answered the way the
//! decision record that allows it says: only the operator may make it, minting asks for
//! the password again in the same request, and the secret appears once, in the reply
//! that minted it, sent with `no-store` as every reply here is. A session left open on a
//! shared screen mints nothing without the password, and a request forged from another
//! page cannot know it.
//!
//! **No key may mint, list or revoke keys, whatever its scope.** A key is the credential
//! most likely to be held somewhere the operator is not, and one that could mint another
//! would be a way to outlive its own revoke.
//!
//! **A secret never crosses a network in the clear.** A mint asked from another machine
//! over a connection the pin does not verify is refused before anything is minted, as a
//! key presented that way is refused before it is looked at.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::response::Response;
use axum::routing::{delete, get};
use axum::{Extension, Json, Router};
use lemonfiber_core::app::Command;
use lemonfiber_core::keys::run::Asked;
use lemonfiber_core::keys::Minter;
use serde::Deserialize;

use crate::admission::Caller;
use crate::guard::Arrived;
use crate::read::carried_out;
use crate::refusal::Refusal;
use crate::router::Serving;

/// Where keys are listed and minted.
pub const KEYS: &str = "/api/keys";

/// Where one key is revoked, by its name.
pub const A_KEY: &str = "/api/keys/{name}";

/// What a mint is asked with.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Minting {
    /// What to call it.
    name: String,
    /// What it admits: `read`, `act` or `member:<account>`.
    scope: String,
    /// What it is for: `home-assistant`, `mcp` or `other`.
    purpose: String,
    /// The operator's password, given again.
    password: String,
}

/// What a mint asked for, the password withheld.
impl std::fmt::Debug for Minting {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Minting")
            .field("name", &self.name)
            .field("scope", &self.scope)
            .field("purpose", &self.purpose)
            .field("password", &"withheld")
            .finish()
    }
}

/// The three routes keys are kept through.
pub fn routes() -> Router<Serving> {
    Router::new()
        .route(KEYS, get(listed).post(minted))
        .route(A_KEY, delete(revoked))
}

/// Every key, without its secret.
async fn listed(State(serving): State<Serving>, caller: Caller) -> Response {
    if let Err(refused) = operator(&caller) {
        return *refused;
    }
    carried_out(&serving.ctx, Command::Keys(Asked::List)).await
}

/// A key minted, its secret shown this once.
async fn minted(
    State(serving): State<Serving>,
    caller: Caller,
    arrived: Option<Extension<Arrived>>,
    given: Result<Json<Minting>, JsonRejection>,
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
    let now = serving.ctx.seams.clock.now();
    match serving
        .admitting
        .proves_the_operator(&given.password, Some(arrived.from), now)
        .await
    {
        Ok(true) => {}
        Ok(false) => return Refusal::NotThePassword.answered(),
        Err(left) => return crate::admission::waiting(left.as_secs().max(1)),
    }
    let asked = Asked::Mint {
        name: given.name,
        scope: given.scope,
        purpose: given.purpose,
        by: Minter::Operator,
    };
    carried_out(&serving.ctx, Command::Keys(asked)).await
}

/// A key revoked, by its name.
async fn revoked(
    State(serving): State<Serving>,
    caller: Caller,
    Path(name): Path<String>,
) -> Response {
    if let Err(refused) = operator(&caller) {
        return *refused;
    }
    let asked = Asked::Revoke {
        name,
        by: Minter::Operator,
    };
    carried_out(&serving.ctx, Command::Keys(asked)).await
}

/// Whether this caller may keep keys, or the refusal they are answered with.
///
/// The operator, by a session or by this run's own token, and nobody else. A key is
/// refused naming what it is, so a program told no knows that no key of any scope will
/// do; anybody else is told it is not theirs.
fn operator(caller: &Caller) -> Result<(), Box<Response>> {
    match caller {
        Caller::Operator | Caller::Machine => Ok(()),
        Caller::Key(_) => Err(Box::new(
            Refusal::NotForAKey
                .saying("A key may not mint, list or revoke keys, whatever its scope."),
        )),
        Caller::Member(_) => Err(Box::new(Refusal::NotYours.answered())),
    }
}

#[cfg(test)]
mod tests;
