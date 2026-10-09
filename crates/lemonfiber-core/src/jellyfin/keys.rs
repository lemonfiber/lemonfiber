//! The API keys Jellyfin holds, and the two of them lemonfiber answers for.
//!
//! An API key on this server administers all of it, on every supported line: the key
//! entity has no scope to narrow. So a key is only ever on the server where something
//! needs one, filed under the name of what holds it, so an operator reading Jellyfin's
//! own key list can tell whose each one is.
//!
//! The one filed as `lemonfiber` is held by nothing, and is taken off. The one filed as
//! `lemonfiber-decline` is held by the decline service alone, which switches off an
//! invitation the invitee refuses, and the one filed as `lemonfiber-request-gate` by the
//! request gate alone, which answers the request service's calls to the media server.

use serde::Deserialize;

use crate::ports::http::Method;
use crate::ports::service::{Dated, Failure};

use super::{carrying, Jellyfin, AUTHORIZATION_HEADER};

/// What the request service files the key it mints for itself under, on the sign-in
/// that sets it up without the request gate in front of the media server.
pub const SEERR_APP: &str = "Seerr";

/// Where the keys are listed, minted and revoked.
const KEYS: &str = "/Auth/Keys";

/// Every key filed under `app`, oldest first, as the server lists them.
///
/// # Errors
///
/// Returns [`Failure`] where Jellyfin is unreachable, refuses the sign-in, or answers
/// the key list with something unreadable.
pub(super) async fn filed_as(jellyfin: &Jellyfin, app: &str) -> Result<Vec<String>, Failure> {
    let response = jellyfin.as_admin(Method::Get, KEYS, None).await?;
    let keys: Keys = jellyfin
        .endpoint
        .decode(&response, "the key list could not be read")?;
    Ok(keys
        .items
        .into_iter()
        .filter(|key| key.app_name == app && !key.access_token.is_empty())
        .map(|key| key.access_token)
        .collect())
}

/// When each key filed under `app` was made and last used, as the server dates each,
/// and none where no key is filed under it.
///
/// Every one is answered, not only the newest: a second key filed under the same name
/// would otherwise hide the uses of the first.
///
/// # Errors
///
/// Returns [`Failure`] where Jellyfin is unreachable, refuses the sign-in, or answers
/// the key list with something unreadable.
pub(super) async fn dated(jellyfin: &Jellyfin, app: &str) -> Result<Vec<Dated>, Failure> {
    let response = jellyfin.as_admin(Method::Get, KEYS, None).await?;
    let keys: Keys = jellyfin
        .endpoint
        .decode(&response, "the key list could not be read")?;
    Ok(keys
        .items
        .into_iter()
        .filter(|key| key.app_name == app && !key.access_token.is_empty())
        .map(|key| Dated {
            created: key.created,
            last_used: key.last_used,
        })
        .collect())
}

/// Mint a key filed under `app`, and answer it.
///
/// Jellyfin answers a mint with no body, so the new key is the one under `app` the
/// list holds after the mint and did not before. Anything other than exactly one is
/// refused rather than guessed at: a key handed over that is not the one just made
/// is a credential somebody else may also hold.
///
/// # Errors
///
/// Returns [`Failure`] where Jellyfin is unreachable, refuses the sign-in or the
/// mint, answers the key list with something unreadable, or lists other than one new
/// key under `app`.
pub(super) async fn mint(jellyfin: &Jellyfin, app: &str) -> Result<String, Failure> {
    let before = filed_as(jellyfin, app).await?;
    let response = jellyfin
        .as_admin(Method::Post, &format!("{KEYS}?App={app}"), None)
        .await?;
    jellyfin.endpoint.expect_success(&response)?;
    let mut new: Vec<String> = filed_as(jellyfin, app)
        .await?
        .into_iter()
        .filter(|key| !before.contains(key))
        .collect();
    match (new.pop(), new.is_empty()) {
        (Some(key), true) => Ok(key),
        _ => Err(jellyfin.endpoint.refused(&format!(
            "a key was minted as {app} and the key list does not show exactly one new one"
        ))),
    }
}

/// Whether Jellyfin answers to `key` alone: its system information, read with the
/// key and no session.
///
/// # Errors
///
/// Returns [`Failure`] where Jellyfin is unreachable or refuses the key.
pub(super) async fn answers_to(jellyfin: &Jellyfin, key: &str) -> Result<(), Failure> {
    let mut asking = jellyfin.request(Method::Get, "/System/Info", None);
    asking
        .headers
        .push((AUTHORIZATION_HEADER.to_owned(), carrying(key)));
    let response = jellyfin.endpoint.send(&asking).await?;
    jellyfin.endpoint.expect_success(&response)
}

/// Revoke the key `key`.
///
/// # Errors
///
/// Returns [`Failure`] where Jellyfin is unreachable, refuses the sign-in, or refuses
/// the revocation.
pub(super) async fn revoke(jellyfin: &Jellyfin, key: &str) -> Result<(), Failure> {
    let response = jellyfin
        .as_admin(Method::Delete, &format!("{KEYS}/{key}"), None)
        .await?;
    jellyfin.endpoint.expect_success(&response)
}

/// The keys Jellyfin holds, as it lists them.
#[derive(Deserialize)]
struct Keys {
    #[serde(rename = "Items", default)]
    items: Vec<Key>,
}

/// One key in that list: what made it, the value itself, and when it was made and
/// last used.
#[derive(Deserialize)]
struct Key {
    #[serde(rename = "AppName", default)]
    app_name: String,
    #[serde(rename = "AccessToken", default)]
    access_token: String,
    #[serde(rename = "DateCreated", default)]
    created: Option<String>,
    #[serde(rename = "DateLastActivity", default)]
    last_used: Option<String>,
}
