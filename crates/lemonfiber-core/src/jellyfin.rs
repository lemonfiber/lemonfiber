//! Driving Jellyfin's first-run setup.
//!
//! Jellyfin is the one service lemonfiber sets an account on rather than reading
//! a key from: it writes no key to disk and asks for its first account through a
//! setup wizard whose endpoints answer only until setup completes. So this reads
//! whether the wizard is done and, where it is not, creates the administrator
//! with a password lemonfiber minted and finishes the wizard — after which those
//! endpoints stop answering, which is what makes the whole thing safe to drive
//! exactly once.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::{Failure, Library, MediaServer};
use crate::recyclarr::Kind;

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

    /// Sign in as the household admin and return the access token the library reads carry.
    /// Jellyfin mints the token from a username and password rather than a stored key, so
    /// a read is always this exchange first, then the read under the token it hands back.
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

/// The one field of the public system info lemonfiber reads: whether the setup
/// wizard has already run. Named as Jellyfin sends it, in `PascalCase`.
#[derive(Deserialize)]
struct PublicInfo {
    #[serde(rename = "StartupWizardCompleted", default)]
    startup_wizard_completed: bool,
}

/// The one field of a sign-in lemonfiber reads: the access token every later read
/// carries. Named as Jellyfin sends it, in `PascalCase`.
#[derive(Deserialize)]
struct Session {
    #[serde(rename = "AccessToken", default)]
    access_token: String,
}

/// A page of library items, as Jellyfin returns them under `Items`.
#[derive(Deserialize)]
struct ItemPage {
    #[serde(rename = "Items", default)]
    items: Vec<LibraryItem>,
}

/// The one field of a library item a trace reads: its title, to match the term against.
#[derive(Deserialize)]
struct LibraryItem {
    #[serde(rename = "Name", default)]
    name: String,
}

/// Jellyfin's own name for the item type a [`Kind`] traces — a series for television, a
/// movie for film — the value its `IncludeItemTypes` filter narrows the library by.
const fn item_type(kind: Kind) -> &'static str {
    match kind {
        Kind::Sonarr => "Series",
        Kind::Radarr => "Movie",
    }
}

#[async_trait]
impl MediaServer for Jellyfin {
    async fn startup_completed(&self) -> Result<bool, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, "/System/Info/Public", None))
            .await?;
        let info: PublicInfo = self
            .endpoint
            .decode(&response, "the public system info could not be read")?;
        Ok(info.startup_wizard_completed)
    }

    async fn create_admin(&self, name: &str, password: &str) -> Result<(), Failure> {
        let body = serde_json::json!({ "Name": name, "Password": password }).to_string();
        let created = self
            .endpoint
            .send(&self.request(Method::Post, "/Startup/User", Some(body)))
            .await?;
        self.endpoint.expect_success(&created)?;

        let completed = self
            .endpoint
            .send(&self.request(Method::Post, "/Startup/Complete", None))
            .await?;
        self.endpoint.expect_success(&completed)
    }
}

#[async_trait]
impl Library for Jellyfin {
    async fn has_item(&self, kind: Kind, term: &str) -> Result<bool, Failure> {
        let token = self.sign_in().await?;

        // Narrow to the media type and recurse into every library folder, then match the
        // term in the same case-insensitive, contains way the *arr found the item by — so
        // the two ends of the trace agree on what "a title matches" means. The token, not
        // a query string, carries the term, so nothing here has to be URL-encoded.
        let path = format!("/Items?Recursive=true&IncludeItemTypes={}", item_type(kind));
        let mut request = self.request(Method::Get, &path, None);
        request.headers.push((TOKEN_HEADER.to_owned(), token));
        let response = self.endpoint.send(&request).await?;
        let page: ItemPage = self
            .endpoint
            .decode(&response, "the library could not be read")?;

        let needle = term.to_lowercase();
        Ok(page
            .items
            .iter()
            .any(|item| item.name.to_lowercase().contains(&needle)))
    }
}
