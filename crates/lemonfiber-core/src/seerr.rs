//! Configuring Seerr to authenticate against Jellyfin.
//!
//! **Seerr is told where Jellyfin is in pieces, not as an address.** It assembles the
//! address itself from a scheme, host, port and base path, so it is given those and
//! not the one string every other client here is handed. That is Seerr's peculiarity
//! rather than the household's, which is why taking the address apart happens here.
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
use crate::ports::service::{
    Failure, FulfilmentTarget, HouseholdRequest, RegisteredTarget, Requests, Telling,
};
use crate::recyclarr::Kind;

/// Seerr's own API key, read from the settings file it writes.
///
/// The manifest declares this key as one retrieved over Seerr's own API, and it can
/// be — but Seerr also writes it to `settings.json`, where reading it costs no
/// authenticated call at all. The file is the cheaper answer to the same question,
/// and the one used here.
///
/// Nothing where the file holds no key yet: Seerr writes one when it is initialised,
/// which is the run that gives it an owner, so a stack seeded before that has none to
/// publish rather than an empty one to publish wrongly.
#[must_use]
pub fn api_key(settings_json: &str) -> Option<String> {
    let settings: serde_json::Value = serde_json::from_str(settings_json).ok()?;
    let key = settings.get("main")?.get("apiKey")?.as_str()?;
    (!key.is_empty()).then(|| key.to_owned())
}

/// The address a fresh Seerr owner is filed under. Seerr requires an address on
/// the initialising sign-in but derives the real one from Jellyfin, so a stable
/// placeholder in lemonfiber's own domain is enough.
const OWNER_EMAIL: &str = "admin@lemonfiber.local";

/// Selects Jellyfin as the media server, as Seerr numbers its kinds — Jellyfin,
/// not Plex or Emby.
const JELLYFIN_SERVER_TYPE: u8 = 2;

/// Where the media server is, in the pieces Seerr asks for it in.
///
/// **Seerr assembles the address itself** — scheme, host, port and base path, joined
/// as `{scheme}://{host}:{port}{base}` — so it takes the parts rather than an
/// address. Handed a whole one it builds `http://http://host:8096:undefined` and
/// refuses that as an invalid address, which is a refusal about a URL nobody wrote.
///
/// Taking it apart is Seerr's peculiarity rather than the household's, so it happens
/// here: what the rest of this workspace passes around stays the address a service is
/// reached at, the way every other client here is given one.
struct Reached<'a> {
    /// Whether it is reached over TLS.
    secure: bool,
    /// The host on its own, with no scheme and no port.
    host: &'a str,
    /// The port it answers on.
    port: u16,
    /// A path the service is served under, empty where it is served at the root.
    base: String,
}

/// The port assumed where an address carries none, by its scheme.
const PLAIN: u16 = 80;
/// The same, for TLS.
const SECURE: u16 = 443;

/// Take an address apart into the pieces Seerr asks for.
///
/// Nothing where the scheme is one this cannot speak: a refusal naming the address
/// is worth more than a request built from a guess about it.
fn taken_apart(server_url: &str) -> Option<Reached<'_>> {
    let (secure, rest) = server_url
        .strip_prefix("https://")
        .map(|rest| (true, rest))
        .or_else(|| server_url.strip_prefix("http://").map(|rest| (false, rest)))?;
    let (authority, base) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, String::new()),
    };
    // A service named without a port answers on the one its scheme implies. Seerr
    // writes the port into the address whatever it is, so there is no leaving it out.
    let assumed = if secure { SECURE } else { PLAIN };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => port
            .parse()
            .map_or((authority, assumed), |port| (host, port)),
        None => (authority, assumed),
    };
    (!host.is_empty()).then_some(Reached {
        secure,
        host,
        port,
        base,
    })
}

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

/// Where the \*arrs that fetch what the household asks for are registered.
///
/// Two lists, not one: the request service keeps film and television apart because
/// they are fetched by different services, and which list a target belongs in is
/// intrinsic to which \*arr it is.
const FILM: &str = "/settings/radarr";
const TELEVISION: &str = "/settings/sonarr";

/// Where media-server accounts are given accounts here.
///
/// It reads the media server itself, using the credentials this service was set up
/// with, so what is sent is identifiers and not accounts. **A member it already holds
/// is skipped**, which is what lets every member be sent on every run.
const LINK_MEMBERS: &str = "/user/import-from-jellyfin";

/// How available a film must be before it is fetched.
///
/// The service's own vocabulary. Released is the one that matches what a household
/// means by asking for something: in cinemas is not something anybody can watch at
/// home, and announced is not something that exists yet.
const WHEN_RELEASED: &str = "released";

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

    /// Send a sign-in and keep whatever session it leaves.
    ///
    /// The session cookie it sets is carried by the transport, which is what
    /// authorises everything read afterwards.
    async fn opened(&self, body: String) -> Result<(), Failure> {
        let signed_in = self
            .endpoint
            .send(&self.request(Method::Post, "/auth/jellyfin", Some(body)))
            .await?;
        self.endpoint.expect_success(&signed_in)
    }

    /// Sign in *and* name where the media server is — the call that sets it.
    ///
    /// Only the first one: a service already pointed at a media server refuses an
    /// address, which is why the session-only sign-in sends none.
    async fn signed_in_naming(
        &self,
        username: &str,
        password: &str,
        server_url: &str,
    ) -> Result<(), Failure> {
        let Some(jellyfin) = taken_apart(server_url) else {
            return Err(self.endpoint.refused(&format!(
                "the media server's address is not one this can take apart for the request \
                 service: {server_url}"
            )));
        };
        let body = serde_json::json!({
            "username": username,
            "password": password,
            "hostname": jellyfin.host,
            "port": jellyfin.port,
            "useSsl": jellyfin.secure,
            "urlBase": jellyfin.base,
            "email": OWNER_EMAIL,
            "serverType": JELLYFIN_SERVER_TYPE,
        })
        .to_string();
        self.opened(body).await
    }
}

/// The one field of the public settings lemonfiber reads: whether Seerr has been
/// initialised.
#[derive(Deserialize)]
struct PublicSettings {
    #[serde(default)]
    initialized: bool,
}

/// An \*arr the request service holds, in its own words.
///
/// Matched on afterwards by host and port rather than by `name`, so an operator who
/// renamed one is not handed a duplicate of it.
#[derive(Deserialize)]
struct TargetResource {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    hostname: String,
    #[serde(default)]
    port: u16,
}

impl TargetResource {
    /// The same target in this product's own words.
    fn registered(self, television: bool) -> RegisteredTarget {
        RegisteredTarget {
            id: self.id.to_string(),
            host: self.hostname,
            port: self.port,
            television,
        }
    }
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

    async fn sign_in(&self, username: &str, password: &str) -> Result<(), Failure> {
        // No address: this is the sign-in for a service already pointed at a media
        // server, and naming one again is how you ask it to be pointed somewhere. It
        // refuses that outright — "hostname already configured" — so a session opened
        // this way is the only one available after the first run, which is every run
        // that matters for reading.
        let body = serde_json::json!({
            "username": username,
            "password": password,
        })
        .to_string();
        self.opened(body).await
    }

    async fn configure_identity(
        &self,
        username: &str,
        password: &str,
        server_url: &str,
    ) -> Result<(), Failure> {
        // Signing in creates the owner and sets the media server, but it does not finish
        // setup — Seerr still reports itself uninitialised until told to.
        self.signed_in_naming(username, password, server_url)
            .await?;

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

    async fn fulfilment_targets(&self) -> Result<Vec<RegisteredTarget>, Failure> {
        let mut held = Vec::new();
        for (path, television) in [(FILM, false), (TELEVISION, true)] {
            let response = self
                .endpoint
                .send(&self.request(Method::Get, path, None))
                .await?;
            let listed: Vec<TargetResource> = self.endpoint.decode(
                &response,
                "the request service's fulfilment targets could not be read",
            )?;
            held.extend(
                listed
                    .into_iter()
                    .map(|target| target.registered(television)),
            );
        }
        Ok(held)
    }

    async fn add_fulfilment_target(&self, target: &FulfilmentTarget) -> Result<(), Failure> {
        let mut body = serde_json::json!({
            "name": target.name,
            "hostname": target.host,
            "port": target.port,
            "apiKey": target.key,
            "useSsl": false,
            "activeProfileId": target.profile.id,
            "activeProfileName": target.profile.name,
            "activeDirectory": target.folder,
            "is4k": false,
            "isDefault": true,
        });

        // The last field is the one the two lists do not share, and each requires its
        // own: television is filed in folders per season, and a film has a point before
        // which there is nothing to fetch. Sending the wrong one is not a field ignored
        // — the service refuses the registration for the one that is missing.
        if let Some(fields) = body.as_object_mut() {
            let (name, value) = if target.television {
                // Seasons in folders of their own, because that is how the media server
                // reads a series and how anybody browsing one expects to find it.
                ("enableSeasonFolders", serde_json::json!(true))
            } else {
                ("minimumAvailability", serde_json::json!(WHEN_RELEASED))
            };
            fields.insert(name.to_owned(), value);
        }
        let body = body.to_string();
        let path = if target.television { TELEVISION } else { FILM };
        let written = self
            .endpoint
            .send(&self.request(Method::Post, path, Some(body)))
            .await?;
        self.endpoint.expect_success(&written)
    }

    async fn link_members(&self, members: &[String]) -> Result<(), Failure> {
        // Nothing to say rather than an empty import: a request naming nobody is one
        // the service has no reason to answer, and a run with no members is ordinary
        // on a stack whose media server could not be read.
        if members.is_empty() {
            return Ok(());
        }
        let body = serde_json::json!({ "jellyfinUserIds": members }).to_string();
        let written = self
            .endpoint
            .send(&self.request(Method::Post, LINK_MEMBERS, Some(body)))
            .await?;
        self.endpoint.expect_success(&written)
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
