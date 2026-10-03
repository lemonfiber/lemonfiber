//! The API keys Jellyfin holds, and the two of them lemonfiber answers for.
//!
//! An API key on this server administers all of it, on every supported line: the key
//! entity has no scope to narrow. So a key is only ever on the server where something
//! needs one, filed under the name of what holds it, so an operator reading Jellyfin's
//! own key list can tell whose each one is.
//!
//! The one filed as `lemonfiber` is held by nothing, and is taken off. The one filed as
//! `lemonfiber-decline` is held by the decline service alone, which switches off an
//! invitation the invitee refuses.

use serde::Deserialize;

use crate::ports::http::Method;
use crate::ports::service::Failure;

use super::{carrying, Jellyfin, AUTHORIZATION_HEADER};

/// The name lemonfiber files its own API key under.
const APP: &str = "lemonfiber";

/// The name the decline service's key is filed under.
pub const DECLINE_APP: &str = "lemonfiber-decline";

/// Where the keys are listed, minted and revoked.
const KEYS: &str = "/Auth/Keys";

impl Jellyfin {
    /// Revoke the API key filed under lemonfiber's name, where there is one.
    ///
    /// No part of the stack reads one: the dashboard is published to the household
    /// network, which is not somewhere a key that administers the server is kept. A key
    /// on the server is a credential nothing needs, so it is taken off rather than left
    /// valid.
    ///
    /// Answers whether one was there to revoke.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where Jellyfin is unreachable, refuses the sign-in, answers
    /// the key list with something unreadable, or refuses the revocation.
    pub async fn revoke_our_key(&self) -> Result<bool, Failure> {
        let Some(ours) = self.filed_as(APP).await?.into_iter().next() else {
            return Ok(false);
        };
        self.revoke(&ours).await?;
        Ok(true)
    }

    /// Every key filed under `app`, oldest first, as the server lists them.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where Jellyfin is unreachable, refuses the sign-in, or answers
    /// the key list with something unreadable.
    pub async fn filed_as(&self, app: &str) -> Result<Vec<String>, Failure> {
        let request = self.as_admin(Method::Get, KEYS, None).await?;
        let response = self.endpoint.send(&request).await?;
        let keys: Keys = self
            .endpoint
            .decode(&response, "the key list could not be read")?;
        Ok(keys
            .items
            .into_iter()
            .filter(|key| key.app_name == app && !key.access_token.is_empty())
            .map(|key| key.access_token)
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
    pub async fn mint(&self, app: &str) -> Result<String, Failure> {
        let before = self.filed_as(app).await?;
        let minting = self
            .as_admin(Method::Post, &format!("{KEYS}?App={app}"), None)
            .await?;
        let response = self.endpoint.send(&minting).await?;
        self.endpoint.expect_success(&response)?;
        let mut new: Vec<String> = self
            .filed_as(app)
            .await?
            .into_iter()
            .filter(|key| !before.contains(key))
            .collect();
        match (new.pop(), new.is_empty()) {
            (Some(key), true) => Ok(key),
            _ => Err(self.endpoint.refused(&format!(
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
    pub async fn answers_to(&self, key: &str) -> Result<(), Failure> {
        let mut asking = self.request(Method::Get, "/System/Info", None);
        asking
            .headers
            .push((AUTHORIZATION_HEADER.to_owned(), carrying(key)));
        let response = self.endpoint.send(&asking).await?;
        self.endpoint.expect_success(&response)
    }

    /// Revoke the key `key`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where Jellyfin is unreachable, refuses the sign-in, or refuses
    /// the revocation.
    pub async fn revoke(&self, key: &str) -> Result<(), Failure> {
        let revoking = self
            .as_admin(Method::Delete, &format!("{KEYS}/{key}"), None)
            .await?;
        let response = self.endpoint.send(&revoking).await?;
        self.endpoint.expect_success(&response)
    }
}

/// The keys Jellyfin holds, as it lists them.
#[derive(Deserialize)]
struct Keys {
    #[serde(rename = "Items", default)]
    items: Vec<Key>,
}

/// One key in that list: what made it, and the value itself.
#[derive(Deserialize)]
struct Key {
    #[serde(rename = "AppName", default)]
    app_name: String,
    #[serde(rename = "AccessToken", default)]
    access_token: String,
}
