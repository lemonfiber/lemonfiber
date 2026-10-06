//! Minting, listing and revoking keys from a browser or the companion.
//!
//! The one credential write this surface takes, and it is answered the way the
//! decision record that allows it says: the operator keeps every key, minting asks for
//! the password again in the same request, and the secret appears once, in the reply
//! that minted it, sent with `no-store` as every reply here is. A session left open on a
//! shared screen mints nothing without the password, and a request forged from another
//! page cannot know it.
//!
//! **A household member keeps keys of their own once the operator allows it**, through
//! the same three routes and the same guard: their own password again in the same
//! request, and a key scoped to them alone. What a member may see, mint and revoke is
//! decided by the core, beside the operator's.
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
use lemonfiber_core::keys::run::members_may_mint;
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
    match keeper(&caller) {
        Ok(by) => carried_out(&serving.ctx, Command::Keys(Asked::List { by })).await,
        Err(refused) => *refused,
    }
}

/// A key minted, its secret shown this once.
async fn minted(
    State(serving): State<Serving>,
    caller: Caller,
    arrived: Option<Extension<Arrived>>,
    given: Result<Json<Minting>, JsonRejection>,
) -> Response {
    let by = match keeper(&caller) {
        Ok(by) => by,
        Err(refused) => return *refused,
    };
    let Some(Extension(arrived)) = arrived.filter(|Extension(arrived)| arrived.may_carry_a_key())
    else {
        return Refusal::KeyInTheClear.answered();
    };
    let Ok(Json(given)) = given else {
        return Refusal::NotAKeyRequest.answered();
    };
    let asked = Asked::Mint {
        name: given.name,
        scope: given.scope,
        purpose: given.purpose,
        by: by.clone(),
    };
    // A member the operator has not allowed is told so before their password is put to
    // the media server: the answer does not turn on it, and asking would open a sign-in
    // there for nothing.
    if matches!(by, Minter::Member { .. }) && !members_may_mint(&serving.ctx) {
        return carried_out(&serving.ctx, Command::Keys(asked)).await;
    }
    let now = serving.ctx.seams.clock.now();
    let proved = match &by {
        Minter::Operator => {
            serving
                .admitting
                .proves_the_operator(&given.password, Some(arrived.from), now)
                .await
        }
        Minter::Member { id } => {
            serving
                .admitting
                .proves_the_member(
                    id,
                    &given.password,
                    Some(arrived.from),
                    now,
                    serving.ctx.seams.random.as_ref(),
                )
                .await
        }
    };
    match proved {
        Ok(true) => carried_out(&serving.ctx, Command::Keys(asked)).await,
        Ok(false) => Refusal::NotThePassword.answered(),
        Err(left) => crate::admission::waiting(left.as_secs().max(1)),
    }
}

/// A key revoked, by its name.
async fn revoked(
    State(serving): State<Serving>,
    caller: Caller,
    Path(name): Path<String>,
) -> Response {
    match keeper(&caller) {
        Ok(by) => carried_out(&serving.ctx, Command::Keys(Asked::Revoke { name, by })).await,
        Err(refused) => *refused,
    }
}

/// Who this caller keeps keys as, or the refusal they are answered with.
///
/// The operator, by a session or by this run's own token, keeps every key; a member keeps
/// their own, as far as the core allows. A key is refused naming what it is, so a program
/// told no knows that no key of any scope will do.
fn keeper(caller: &Caller) -> Result<Minter, Box<Response>> {
    match caller {
        Caller::Operator | Caller::Machine => Ok(Minter::Operator),
        Caller::Member(id) => Ok(Minter::Member { id: id.clone() }),
        Caller::Key(_) => Err(Box::new(
            Refusal::NotForAKey
                .saying("A key may not mint, list or revoke keys, whatever its scope."),
        )),
    }
}

#[cfg(test)]
mod tests;
