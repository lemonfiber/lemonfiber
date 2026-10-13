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
use lemonfiber_core::device_keys;
use lemonfiber_core::device_keys::proof::Mint;
use lemonfiber_core::keys::run::members_may_mint;
use lemonfiber_core::keys::run::Asked;
use lemonfiber_core::keys::Minter;
use serde::Deserialize;

use crate::admission::{Caller, Door};
use crate::guard::{Arrived, Presented};
use crate::read::{carried_out, went_wrong};
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
    let mint = Mint {
        name: &given.name,
        scope: &given.scope,
        purpose: &given.purpose,
        claim: given.claim,
    };
    let proof = proof(&given);
    if let (Some(Proof::Device(device)), Minter::Operator) = (&proof, &by) {
        if let Err(refused) =
            proved_by_device(&serving, &presented, arrived.from, device, &mint).await
        {
            return refused;
        }
    }
    let proved_by = match &proof {
        Some(Proof::Device(device)) => Some(device.name.clone()),
        Some(Proof::Password(_)) | None => None,
    };
    let asked = Asked::Mint {
        name: given.name,
        scope: given.scope,
        purpose: given.purpose,
        by: by.clone(),
        claim: given.claim,
        proved_by,
    };
    let password = match (proof, &by) {
        (None, _) => return Refusal::NotAKeyRequest.answered(),
        (Some(Proof::Device(_)), Minter::Member { .. }) => {
            return Refusal::NotYours.saying("Device keys are the operator's, and only theirs.")
        }
        (Some(Proof::Device(_)), Minter::Operator) => {
            return carried_out(&serving.ctx, Command::Keys(asked)).await;
        }
        (Some(Proof::Password(password)), _) => password,
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
                .proves_the_operator(&password, Some(arrived.from), now)
                .await
        }
        Minter::Member { id } => {
            serving
                .admitting
                .proves_the_member(
                    id,
                    &password,
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

/// What proves the operator, or the member, for a mint.
enum Proof {
    /// The password, given again.
    Password(String),
    /// A device key's signature over a challenge.
    Device(Device),
}

/// A device key's proof, as a mint carries it.
struct Device {
    /// The device key's name.
    name: String,
    /// The challenge it signed.
    challenge: String,
    /// Its signature.
    signature: String,
}

/// The one proof a mint carries, or nothing where it carries none or both.
fn proof(given: &Minting) -> Option<Proof> {
    match (
        &given.password,
        &given.device,
        &given.challenge,
        &given.signature,
    ) {
        (Some(password), None, None, None) => Some(Proof::Password(password.clone())),
        (None, Some(name), Some(challenge), Some(signature)) => Some(Proof::Device(Device {
            name: name.clone(),
            challenge: challenge.clone(),
            signature: signature.clone(),
        })),
        _ => None,
    }
}

/// Whether `device` proves the operator for `mint`, asked with the credential `presented`
/// from `from`: nothing to check further where it does, and the refusal where not.
///
/// An unregistered or revoked device key is refused before the challenge is looked at,
/// and costs no attempt. A challenge that does not stand is spent and costs none either:
/// it was issued, or it was not, and either way it is no guess at anything. A signature
/// that does not verify is counted as a wrong password is, and alerts the operator.
async fn proved_by_device(
    serving: &Serving,
    presented: &Presented,
    from: std::net::IpAddr,
    device: &Device,
    mint: &Mint<'_>,
) -> Result<(), Response> {
    let ctx = &serving.ctx;
    let record = device_keys::run::standing(ctx, &device.name)
        .await
        .map_err(|refused| went_wrong(&refused))?;
    let now = ctx.seams.clock.now();
    let attempts = &serving.admitting.attempts;
    let ticket = attempts
        .taken(Some(from), None, now)
        .await
        .map_err(|left| crate::admission::waiting(left.as_secs().max(1)))?;
    let stood = serving
        .admitting
        .challenges
        .spent(&device.challenge, &presented.0, &device.name, now)
        .await;
    if !stood {
        attempts.forgiven(&ticket).await;
        return Err(went_wrong(&device_keys::run::challenge_not_standing()));
    }
    let signed = record
        .key()
        .is_some_and(|key| mint.proved(&device.challenge, &device.signature, &key));
    if !signed {
        device_keys::run::misproved(ctx, &record).await;
        return Err(went_wrong(&device_keys::run::signature_wrong()));
    }
    attempts.right(&ticket, Door::Operator, now).await;
    device_keys::run::used(ctx, &device.name)
        .await
        .map_err(|refused| went_wrong(&refused))
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
    let now = serving.ctx.seams.clock.now();
    let ticket = match serving
        .admitting
        .attempts
        .taken_at_a_key(arrived.from, now)
        .await
    {
        Ok(ticket) => ticket,
        Err(left) => return crate::admission::waiting(left.as_secs().max(1)),
    };
    let asked = Asked::Collect {
        claim: given.claim,
        from: Some(arrived.from.to_canonical().to_string()),
    };
    let answered = carried_out(&serving.ctx, Command::Keys(asked)).await;
    if answered.status().is_success() {
        serving
            .admitting
            .attempts
            .right(&ticket, Door::Key, now)
            .await;
    }
    answered
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
