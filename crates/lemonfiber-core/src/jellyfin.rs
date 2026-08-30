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

mod household;
mod library;
mod setup;

/// The header Jellyfin identifies a client through on the sign-in that mints an access
/// token — its own scheme, named as it parses it. The values only have to be present and
/// stable; lemonfiber names itself so a household recognises the session on its account.
const AUTHORIZATION: &str =
    r#"MediaBrowser Client="lemonfiber", Device="lemonfiber", DeviceId="lemonfiber", Version="1""#;

/// The header carrying the access token on every read after sign-in.
const TOKEN_HEADER: &str = "X-Emby-Token";

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
        request.headers.push((TOKEN_HEADER.to_owned(), token));
        Ok(request)
    }

    /// The API key the dashboard authenticates with, minted once and reused after.
    ///
    /// Jellyfin writes no key to a file — it keeps them in its own database — so this
    /// is the one credential in the stack that has to be asked for rather than read.
    /// It is minted under lemonfiber's own name so an operator can see which key is
    /// whose, and a key already under that name is handed back rather than a second
    /// one made: a fresh key every seed would leave the old ones behind for as long
    /// as the stack runs.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where Jellyfin is unreachable, refuses the sign-in, or
    /// answers the key list with something unreadable.
    pub async fn api_key(&self) -> Result<String, Failure> {
        if let Some(existing) = self.our_key().await? {
            return Ok(existing);
        }
        let minting = self
            .as_admin(Method::Post, &format!("/Auth/Keys?App={APP}"), None)
            .await?;
        let response = self.endpoint.send(&minting).await?;
        self.endpoint.expect_success(&response)?;
        // Asked for again rather than read from the answer: the mint replies with no
        // body at all, so the key it made is only knowable by listing them.
        self.our_key().await?.ok_or_else(|| {
            self.endpoint
                .refused("the key that was just made is not listed")
        })
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
            .push(("X-Emby-Authorization".to_owned(), AUTHORIZATION.to_owned()));
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
