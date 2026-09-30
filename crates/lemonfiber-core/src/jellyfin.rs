//! Talking to Jellyfin — the media server, and the one service lemonfiber holds an
//! account on rather than a key.
//!
//! Jellyfin writes no key to disk and asks for its first account through a setup wizard
//! whose endpoints answer only until that wizard completes. So lemonfiber drives the
//! wizard once, mints the administrator, and afterwards signs in as that administrator
//! for everything else. This file is the client and the plumbing all of that shares; the
//! two things done with it — driving the first run, and reading and rescanning the
//! library — are a file each beside it.

use std::sync::Arc;

use serde::Deserialize;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::Failure;
use crate::recyclarr::Kind;

mod cors;
mod household;
mod library;
mod setup;

/// The header Jellyfin identifies a client through on the sign-in that mints an access
/// token — its own scheme, named as it parses it. The values only have to be present and
/// stable; lemonfiber names itself so a household recognises the session on its account.
const AUTHORIZATION: &str =
    r#"MediaBrowser Client="lemonfiber", Device="lemonfiber", DeviceId="lemonfiber", Version="1""#;

/// The header both the sign-in and every read after it are carried in.
///
/// **The one scheme every supported line accepts.** Measured side by side on `10.10.3`,
/// `10.11.11` and `12.1`: `12.1` answers a sign-in carried in `X-Emby-Authorization` with
/// `400` and a token in `X-Emby-Token` with `401`, and all three accept both in this header.
const AUTHORIZATION_HEADER: &str = "Authorization";

/// The access a read carries, in the scheme [`AUTHORIZATION_HEADER`] is parsed by.
fn carrying(token: &str) -> String {
    format!(r#"MediaBrowser Token="{token}""#)
}

/// A client for one Jellyfin — its first-run setup, and, once lemonfiber holds the
/// household's admin credential, reading its library to answer a trace.
pub struct Jellyfin {
    endpoint: Endpoint,
    /// The household's admin credential, for the library reads a trace makes. Empty on a
    /// setup-only client, which never signs in — the setup endpoints take no key.
    username: String,
    password: String,
}

impl Jellyfin {
    /// A setup client for the Jellyfin reached at `base`, named `service` — the first-run
    /// driver, which carries no credential because the setup endpoints take none.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>, service: impl Into<String>) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, service),
            username: String::new(),
            password: String::new(),
        }
    }

    /// A reading client for the Jellyfin reached at `base`, signing in as the household
    /// admin lemonfiber minted — the credential a trace's library read authenticates with.
    #[must_use]
    pub fn authenticated(
        http: Arc<dyn Http>,
        base: impl Into<String>,
        service: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, service),
            username: username.into(),
            password: password.into(),
        }
    }

    /// A request to a path on Jellyfin. The setup endpoints are unauthenticated —
    /// there is no key yet, which is the whole reason lemonfiber is here — and a
    /// JSON body is declared as such so it is bound rather than refused.
    fn request(&self, method: Method, path: &str, body: Option<String>) -> Request {
        self.endpoint.json_request(method, path, body)
    }

    /// A request signed in as the household admin, which every read and every rescan is.
    ///
    /// Jellyfin mints its token from a username and password rather than a stored key, so
    /// each of these is the sign-in exchange first and then the request under the token it
    /// hands back.
    async fn as_admin(
        &self,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<Request, Failure> {
        let token = self.sign_in().await?;
        let mut request = self.request(method, path, body);
        request
            .headers
            .push((AUTHORIZATION_HEADER.to_owned(), carrying(&token)));
        Ok(request)
    }

    /// Revoke the API key filed under lemonfiber's name, where there is one.
    ///
    /// No part of the stack reads one: an API key on this server administers all of it,
    /// and the dashboard is published to the household network, which is not somewhere
    /// one is kept. A key on the server is a credential nothing needs, so it is taken off
    /// rather than left valid.
    ///
    /// Answers whether one was there to revoke.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where Jellyfin is unreachable, refuses the sign-in, answers
    /// the key list with something unreadable, or refuses the revocation.
    pub async fn revoke_our_key(&self) -> Result<bool, Failure> {
        let Some(ours) = self.our_key().await? else {
            return Ok(false);
        };
        let revoking = self
            .as_admin(Method::Delete, &format!("/Auth/Keys/{ours}"), None)
            .await?;
        let response = self.endpoint.send(&revoking).await?;
        self.endpoint.expect_success(&response)?;
        Ok(true)
    }

    /// The key already filed under lemonfiber's name, where there is one.
    async fn our_key(&self) -> Result<Option<String>, Failure> {
        let request = self.as_admin(Method::Get, "/Auth/Keys", None).await?;
        let response = self.endpoint.send(&request).await?;
        let keys: Keys = self
            .endpoint
            .decode(&response, "the key list could not be read")?;
        Ok(keys
            .items
            .into_iter()
            .find(|key| key.app_name == APP && !key.access_token.is_empty())
            .map(|key| key.access_token))
    }

    /// Sign in as the household admin and return the access token the reads carry.
    async fn sign_in(&self) -> Result<String, Failure> {
        let body =
            serde_json::json!({ "Username": self.username, "Pw": self.password }).to_string();
        let mut request = self.request(Method::Post, "/Users/AuthenticateByName", Some(body));
        request
            .headers
            .push((AUTHORIZATION_HEADER.to_owned(), AUTHORIZATION.to_owned()));
        let response = self.endpoint.send(&request).await?;
        let session: Session = self
            .endpoint
            .decode(&response, "the sign-in was not accepted")?;
        Ok(session.access_token)
    }
}

/// The name lemonfiber files its own API key under, so an operator reading Jellyfin's
/// key list can tell which key is whose.
const APP: &str = "lemonfiber";

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

/// The one field of a sign-in lemonfiber reads: the access token every later read
/// carries. Named as Jellyfin sends it, in `PascalCase`.
#[derive(Deserialize)]
struct Session {
    #[serde(rename = "AccessToken", default)]
    access_token: String,
}

/// Jellyfin's own name for the item type a [`Kind`] traces — a series for television, a
/// movie for film — the value its `IncludeItemTypes` filter narrows the library by.
const fn item_type(kind: Kind) -> &'static str {
    match kind {
        Kind::Sonarr => "Series",
        Kind::Radarr => "Movie",
    }
}
