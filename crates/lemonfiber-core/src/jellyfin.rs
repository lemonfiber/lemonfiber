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
use crate::ports::http::{Http, Method, Request, Response};
use crate::ports::service::Failure;
use crate::recyclarr::Kind;

mod cors;
mod household;
mod item;
mod keys;
mod library;
mod password;
mod proxies;
mod screening;
mod serving;
mod sessions;
mod setup;

pub use keys::SEERR_APP;
pub use sessions::Sessions;

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

/// A request carrying a token.
fn carried(request: &Request, token: &str) -> Request {
    let mut carried = request.clone();
    carried
        .headers
        .push((AUTHORIZATION_HEADER.to_owned(), carrying(token)));
    carried
}

/// A client for one Jellyfin — its first-run setup, and, once lemonfiber holds the
/// household's admin credential, reading its library to answer a trace.
pub struct Jellyfin {
    endpoint: Endpoint,
    /// The household's admin credential, for the library reads a trace makes. Empty on a
    /// setup-only client, which never signs in — the setup endpoints take no key.
    username: String,
    password: String,
    /// The sessions already signed in to, so a read does not sign in again for each
    /// request it makes.
    sessions: Arc<Sessions>,
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
            sessions: Arc::default(),
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
            sessions: Arc::default(),
        }
    }

    /// The same client, signing in through sessions that outlive it.
    ///
    /// A client is built for each reading, and a reading every few seconds would
    /// otherwise sign in afresh every few seconds — a password hashed by the media
    /// server each time, and a sign-in written to its activity log each time.
    #[must_use]
    pub fn remembering(mut self, sessions: Arc<Sessions>) -> Self {
        self.sessions = sessions;
        self
    }

    /// A request to a path on Jellyfin. The setup endpoints are unauthenticated —
    /// there is no key yet, which is the whole reason lemonfiber is here — and a
    /// JSON body is declared as such so it is bound rather than refused.
    fn request(&self, method: Method, path: &str, body: Option<String>) -> Request {
        self.endpoint.json_request(method, path, body)
    }

    /// A request sent signed in as the household admin, which every read and every
    /// rescan is, and what it answered.
    ///
    /// Jellyfin mints its token from a username and password rather than a stored key.
    /// A token already minted for this server and this credential is carried rather
    /// than minted again, and one the server no longer accepts is minted afresh once
    /// and the request sent again — a server restored from a backup, or a session an
    /// administrator ended, is a token gone stale rather than a credential refused.
    async fn as_admin(
        &self,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<Response, Failure> {
        let request = self.request(method, path, body);
        let held = self
            .sessions
            .held(self.endpoint.url(""), &self.username, &self.password);
        if let Some(token) = held {
            let response = self.endpoint.send(&carried(&request, &token)).await?;
            if !matches!(response.status, 401 | 403) {
                return Ok(response);
            }
            self.sessions
                .forget(self.endpoint.url(""), &self.username, &self.password);
        }
        let token = self.sign_in().await?;
        self.sessions.keep(
            self.endpoint.url(""),
            &self.username,
            &self.password,
            &token,
        );
        self.endpoint.send(&carried(&request, &token)).await
    }

    /// Whether Jellyfin signs the household admin in with the password this client holds.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unauthorised`] where Jellyfin refuses the password, and the
    /// failure itself where it will not answer.
    pub async fn accepts(&self) -> Result<(), Failure> {
        self.sign_in().await.map(|_| ())
    }

    /// Sign in as the household admin and return the access token the reads carry.
    async fn sign_in(&self) -> Result<String, Failure> {
        Ok(self.signed_in(&self.password).await?.access_token)
    }

    /// Sign in as the household admin with `password`, and keep what the sign-in answers.
    async fn signed_in(&self, password: &str) -> Result<Session, Failure> {
        let body = serde_json::json!({ "Username": self.username, "Pw": password }).to_string();
        let mut request = self.request(Method::Post, "/Users/AuthenticateByName", Some(body));
        request
            .headers
            .push((AUTHORIZATION_HEADER.to_owned(), AUTHORIZATION.to_owned()));
        let response = self.endpoint.send(&request).await?;
        self.endpoint
            .decode(&response, "the sign-in was not accepted")
    }
}

/// The two fields of a sign-in lemonfiber reads: the access token every later read
/// carries, and whose account it opened. Named as Jellyfin sends them, in `PascalCase`.
#[derive(Deserialize)]
struct Session {
    #[serde(rename = "AccessToken", default)]
    access_token: String,
    #[serde(rename = "User", default)]
    user: SignedIn,
}

/// The account a sign-in opened.
#[derive(Deserialize, Default)]
struct SignedIn {
    #[serde(rename = "Id", default)]
    id: String,
}

/// Jellyfin's own name for the item type a [`Kind`] traces — a series for television, a
/// movie for film — the value its `IncludeItemTypes` filter narrows the library by.
const fn item_type(kind: Kind) -> &'static str {
    match kind {
        Kind::Tv => "Series",
        Kind::Movies => "Movie",
    }
}
