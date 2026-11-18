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
use axum::routing::{delete, get, post};
use axum::{Extension, Json, Router};
use lemonfiber_core::app::Command;
use lemonfiber_core::keys::run::{Asked, Proof, Proving};
use lemonfiber_core::keys::Minter;
use serde::Deserialize;

use crate::admission::Caller;
use crate::guard::{Arrived, Presented};
use crate::read::carried_out;
use crate::refusal::Refusal;
use crate::router::Serving;

/// Where keys are listed and minted.
pub const KEYS: &str = "/api/keys";

/// Where one key is revoked, by its name.
pub const A_KEY: &str = "/api/keys/{name}";

/// Where a key minted as a claim is collected, by a program holding nothing else.
pub const COLLECTING: &str = "/api/keys/claim";

/// What a mint is asked with.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KeyMinting")]
struct Minting {
    /// What to call it.
    name: String,
    /// What it admits: `read`, `act` or `member:<account>`.
    scope: String,
    /// What it is for: `home-assistant`, `mcp` or `other`.
    purpose: String,
    /// The operator's password, given again; or, from a phone, nothing, and the three
    /// fields that prove the operator with a device key in its place.
    #[serde(default)]
    password: Option<String>,
    /// The name of the device key whose signature stands in for the password.
    #[serde(default)]
    device: Option<String>,
    /// The challenge the core issued for this device key, as it was handed over.
    #[serde(default)]
    challenge: Option<String>,
    /// ECDSA P-256 over SHA-256, in DER, in base64url, of the UTF-8 lines
    /// `lemonfiber-mint/1`, the challenge, the name, the scope, the purpose and `claim` or
    /// `key`, each ended by a line feed.
    #[serde(default)]
    signature: Option<String>,
    /// Whether to mint it as a claim: the reply carries the link a program collects it
    /// with, once, within ten minutes, and no secret.
    #[serde(default)]
    claim: bool,
}

/// What a collection is asked with: the claim alone.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KeyCollecting")]
struct Collecting {
    /// The claim the key link carried.
    claim: String,
}

/// What a collection asked for, the claim withheld.
impl std::fmt::Debug for Collecting {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Collecting")
            .field("claim", &"withheld")
            .finish()
    }
}

/// The body a mint takes, with the route it is sent to, described from the type the
/// route reads it into.
pub(crate) fn body() -> (&'static str, schemars::Schema) {
    (KEYS, schemars::schema_for!(Minting))
}

/// The body a collection takes, with the route it is sent to.
pub(crate) fn collecting_body() -> (&'static str, schemars::Schema) {
    (COLLECTING, schemars::schema_for!(Collecting))
}

/// What a mint asked for, the password withheld.
impl std::fmt::Debug for Minting {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Minting")
            .field("name", &self.name)
            .field("scope", &self.scope)
            .field("purpose", &self.purpose)
            .field("password", &self.password.as_ref().map(|_| "withheld"))
            .field("device", &self.device)
            .field("challenge", &self.challenge.as_ref().map(|_| "withheld"))
            .field("signature", &self.signature.as_ref().map(|_| "withheld"))
            .field("claim", &self.claim)
            .finish()
    }
}

/// The three routes keys are kept through.
pub fn routes() -> Router<Serving> {
    Router::new()
        .route(KEYS, get(listed).post(minted))
        .route(COLLECTING, post(collected))
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
    Extension(presented): Extension<Presented>,
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
    let Some(proof) = proof(&given, &presented) else {
        return Refusal::NotAKeyRequest.answered();
    };
    let ctx = match given.password.clone() {
        Some(password) => crate::admission::typed(&serving.ctx, password),
        None => serving.ctx.as_ref().clone(),
    };
    let asked = Asked::Mint {
        name: given.name,
        scope: given.scope,
        purpose: given.purpose,
        by,
        claim: given.claim,
        proving: Some(Box::new(Proving {
            proof,
            from: Some(arrived.from),
        })),
    };
    carried_out(&ctx, Command::Keys(asked)).await
}

/// What the body offers as proof: the password alone, or a device key's signature over a
/// challenge issued to the session the request carries.
fn proof(given: &Minting, presented: &Presented) -> Option<Proof> {
    match (
        &given.password,
        &given.device,
        &given.challenge,
        &given.signature,
    ) {
        (Some(_), None, None, None) => Some(Proof::Password),
        (None, Some(name), Some(challenge), Some(signature)) => Some(Proof::Device {
            name: name.clone(),
            challenge: challenge.clone(),
            signature: signature.clone(),
            session: presented.0.clone(),
        }),
        _ => None,
    }
}

/// A key minted as a claim, collected with that claim, its secret shown this once.
///
/// The second door that opens without a credential, because the program collecting a
/// key holds nothing else yet. A claim is counted as a guess at a key is, before it is
/// looked at, and only over the connection a key may cross: a secret collected in the
/// clear has been handed to everything on the way.
async fn collected(
    State(serving): State<Serving>,
    arrived: Option<Extension<Arrived>>,
    given: Result<Json<Collecting>, JsonRejection>,
) -> Response {
    let Some(Extension(arrived)) = arrived.filter(|Extension(arrived)| arrived.may_carry_a_key())
    else {
        return Refusal::KeyInTheClear.answered();
    };
    let Ok(Json(given)) = given else {
        return Refusal::NotAKeyRequest.answered();
    };
    let asked = Asked::Collect {
        claim: given.claim,
        from: Some(arrived.from),
    };
    carried_out(&serving.ctx, Command::Keys(asked)).await
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
