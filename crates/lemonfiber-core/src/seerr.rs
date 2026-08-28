//! Configuring Seerr to authenticate against Jellyfin.
//!
//! Seerr (Jellyseerr) is initialised by its first authenticated call: the first
//! account to sign in through Jellyfin becomes its owner, and the media server is
//! set to that Jellyfin at the same time. So this reads whether it is already
//! initialised — which lemonfiber never overrides, since re-pointing a running
//! instance's identity would cost the household its existing sign-ins — and, where
//! it is not, signs in through Jellyfin to point Seerr's authentication at it.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::{Failure, HouseholdRequest, Requests, Telling};
use crate::recyclarr::Kind;

/// The address a fresh Seerr owner is filed under. Seerr requires an address on
/// the initialising sign-in but derives the real one from Jellyfin, so a stable
/// placeholder in lemonfiber's own domain is enough.
const OWNER_EMAIL: &str = "admin@lemonfiber.local";

/// Selects Jellyfin as the media server, as Seerr numbers its kinds — Jellyfin,
/// not Plex or Emby.
const JELLYFIN_SERVER_TYPE: u8 = 2;

/// The occasions the household is told about, as Seerr numbers them.
///
/// A request was received, a decision was made either way, it arrived, or it could
/// not be got. Each is one bit of the field Seerr keeps the set in, named here rather
/// than written as one number so what is being asked for can be read.
///
/// **The one easy to leave out is `AUTO_APPROVED`.** A household whose policy
/// approves automatically never has a *pending* request, so a set built from
/// `PENDING` alone tells that household nothing at the moment they asked — which is
/// exactly the household that most expects the loop to close by itself.
const RECEIVED: u32 = 2;
const DECIDED_YES: u32 = 4;
const ARRIVED: u32 = 8;
const COULD_NOT: u32 = 16;
const DECIDED_NO: u32 = 64;
const APPROVED_BY_POLICY: u32 = 128;

/// Everything the household is told about, taken together.
pub const OCCASIONS: u32 =
    RECEIVED | DECIDED_YES | ARRIVED | COULD_NOT | DECIDED_NO | APPROVED_BY_POLICY;

/// Where the one agent that needs no account of its own is configured.
///
/// Every other agent Seerr offers wants a service to sign in to — a mail server, a
/// chat workspace, a push provider's key. This one is the browser's own, so a
/// household that has done nothing but visit the page can be reached.
const WEBPUSH: &str = "/settings/notifications/webpush";

/// A client for one Seerr's identity setup.
pub struct Seerr {
    endpoint: Endpoint,
}

impl Seerr {
    /// A client for the Seerr reached at `base`, named `service`.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>, service: impl Into<String>) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, service),
        }
    }

    /// A request to a path under Seerr's versioned API. A JSON body is declared as
    /// such, because Seerr's framework only parses a body it is told is JSON and
    /// silently drops one it is not.
    fn request(&self, method: Method, path: &str, body: Option<String>) -> Request {
        self.endpoint
            .json_request(method, &format!("/api/v1{path}"), body)
    }
}

/// The one field of the public settings lemonfiber reads: whether Seerr has been
/// initialised.
#[derive(Deserialize)]
struct PublicSettings {
    #[serde(default)]
    initialized: bool,
}

/// What Seerr holds for the browser-push agent, in its own words.
///
/// `types` is the bit field of occasions. Both default rather than being required,
/// because a Seerr that has never had the agent touched answers with the fields it
/// happens to have and a missing one means off, not unreadable.
#[derive(Deserialize)]
struct WebPush {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    types: u32,
}

#[async_trait]
impl Requests for Seerr {
    async fn initialized(&self) -> Result<bool, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, "/settings/public", None))
            .await?;
        let settings: PublicSettings = self
            .endpoint
            .decode(&response, "the public settings could not be read")?;
        Ok(settings.initialized)
    }

    async fn sign_in(
        &self,
        username: &str,
        password: &str,
        server_url: &str,
    ) -> Result<(), Failure> {
        // Signing in through Jellyfin creates the owner and sets the media server on the
        // first call, and on every later one simply opens a session. The session cookie
        // it sets is carried by the transport, which is what authorises what follows.
        let body = serde_json::json!({
            "username": username,
            "password": password,
            "hostname": server_url,
            "email": OWNER_EMAIL,
            "serverType": JELLYFIN_SERVER_TYPE,
        })
        .to_string();
        let signed_in = self
            .endpoint
            .send(&self.request(Method::Post, "/auth/jellyfin", Some(body)))
            .await?;
        self.endpoint.expect_success(&signed_in)
    }

    async fn configure_identity(
        &self,
        username: &str,
        password: &str,
        server_url: &str,
    ) -> Result<(), Failure> {
        // Signing in creates the owner and sets the media server, but it does not finish
        // setup — Seerr still reports itself uninitialised until told to.
        self.sign_in(username, password, server_url).await?;

        // Finishing setup is the step that marks Seerr initialised.
        let finished = self
            .endpoint
            .send(&self.request(Method::Post, "/settings/initialize", None))
            .await?;
        self.endpoint.expect_success(&finished)
    }

    async fn requests(&self) -> Result<Vec<HouseholdRequest>, Failure> {
        // The owner's session sees every member's requests; a member's own would see
        // only theirs. Read newest first, so a household with more than the horizon
        // keeps the requests still worth asking about rather than its oldest.
        let mut requests = Vec::new();
        let mut skip = 0;
        loop {
            let path =
                format!("/request?take={REQUEST_PAGE}&skip={skip}&sort=added&sortDirection=desc");
            let response = self
                .endpoint
                .send(&self.request(Method::Get, &path, None))
                .await?;
            let page: RequestPage = self
                .endpoint
                .decode(&response, "the household's requests could not be read")?;
            let on_this_page = page.results.len();
            requests.extend(page.results.into_iter().map(RequestRecord::into_request));
            if on_this_page < REQUEST_PAGE || requests.len() >= page.page_info.results {
                break;
            }
            skip += REQUEST_PAGE;
        }
        Ok(requests)
    }

    async fn telling(&self) -> Result<Telling, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, WEBPUSH, None))
            .await?;
        let held: WebPush = self
            .endpoint
            .decode(&response, "what the household is told could not be read")?;
        Ok(Telling {
            enabled: held.enabled,
            occasions: held.types,
        })
    }

    async fn tell(&self, telling: &Telling) -> Result<(), Failure> {
        let body = serde_json::json!({
            "enabled": telling.enabled,
            "types": telling.occasions,
        })
        .to_string();
        let written = self
            .endpoint
            .send(&self.request(Method::Post, WEBPUSH, Some(body)))
            .await?;
        self.endpoint.expect_success(&written)
    }
}

/// How many requests are read per page. Seerr answers ten at a time unless told
/// otherwise, so a household of any size would take a walk; this asks for a generous
/// page and walks on only where the service's own total says there is more.
const REQUEST_PAGE: usize = 100;

/// A page of the household's requests, and the totals that say whether there are more.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestPage {
    #[serde(default)]
    page_info: PageInfo,
    #[serde(default)]
    results: Vec<RequestRecord>,
}

/// How many requests there are in total, so the walk knows when it has them all.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    #[serde(default)]
    results: usize,
}

/// One request as Seerr records it: what became of it, what became of the media it
/// asked for, who asked, and — once it has been handed over — which \*arr item it is.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestRecord {
    #[serde(default)]
    status: u8,
    #[serde(default, rename = "type")]
    media_type: String,
    #[serde(default)]
    media: MediaRecord,
    #[serde(default)]
    requested_by: MemberRecord,
}

/// The media a request asked for. It carries no title — Seerr looks those up from a
/// metadata service rather than storing them — but it does carry the id the \*arr
/// filing it knows it by, which is the exact join a name is found through.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MediaRecord {
    #[serde(default)]
    status: u8,
    #[serde(default)]
    external_service_id: Option<i64>,
}

/// The member who asked, under the display name Seerr shows them by.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemberRecord {
    #[serde(default)]
    display_name: String,
}

impl RequestRecord {
    /// The request as the port carries it, with the media type read into the service
    /// that files it — television or film, or nothing for a kind this build does not know.
    fn into_request(self) -> HouseholdRequest {
        HouseholdRequest {
            member: self.requested_by.display_name,
            kind: match self.media_type.as_str() {
                "tv" => Some(Kind::Sonarr),
                "movie" => Some(Kind::Radarr),
                _ => None,
            },
            item: self.media.external_service_id,
            request_status: self.status,
            media_status: self.media.status,
        }
    }
}
