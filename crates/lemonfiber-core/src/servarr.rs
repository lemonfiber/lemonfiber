//! Speaking the Servarr API shape.
//!
//! Sonarr, Radarr, Lidarr and Prowlarr share one HTTP API, so one client speaks
//! to all of them, told apart only by the address and key it is given. It is
//! built on the HTTP port rather than on a real client, so the same code proves
//! a credential in the doctor and wires services during seed, and both are tested
//! against a fake with no service running.
//!
//! A status code is read for what it means: a refused credential, a service that
//! answered with something unusable, or — through the port — nothing answering at
//! all. The service's own words are carried through rather than paraphrased, so
//! an operator sees what the service said and not a vaguer restatement of it.
//!
//! This file holds the client itself and the provisioning shape (identity,
//! registering clients and folders, queue depth). The reads a trace makes live in
//! [`pipeline`], and the quality reads and writes in [`quality`], so the two newest
//! concerns grow apart from the stable provisioning adapter.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request, Response};
use crate::ports::service::{
    Client, ClientKind, Credential, DownloadClient, Failure, Identity, QueueDepth,
    RegisteredClient, RegisteredFolder, RootFolder,
};

mod pipeline;
mod quality;

/// A client for one Servarr-shape service.
///
/// Holds the endpoint — the transport, the service's address and its name — the
/// key, which is the one thing the Servarr shape adds: a header on every request,
/// and the API version, because the shape spans two of them.
pub struct Servarr {
    endpoint: Endpoint,
    key: String,
    version: u32,
}

impl Servarr {
    /// A client for the service named `service`, reached at `base` with `key`,
    /// speaking its API `version` — v3 for Sonarr and Radarr, v1 for Lidarr.
    #[must_use]
    pub fn new(
        http: Arc<dyn Http>,
        base: impl Into<String>,
        key: impl Into<String>,
        service: impl Into<String>,
        version: u32,
    ) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, service),
            key: key.into(),
            version,
        }
    }

    /// A request to a path under the service's versioned API, carrying the key —
    /// and, where it has a JSON body, declaring it as such so the service binds
    /// it rather than refusing it.
    fn request(&self, method: Method, path: &str, body: Option<String>) -> Request {
        self.endpoint.keyed_request(
            method,
            &format!("/api/v{}{path}", self.version),
            &self.key,
            body,
        )
    }

    /// Send a request to the versioned API, turning a `404` — the whole
    /// `/api/v{version}` prefix not served — into an unsupported-version failure
    /// rather than passing it on as a generic refusal. A service upgraded past (or
    /// standing before) the version this build speaks is then reported as such and
    /// never written to, rather than its 404 read as a rejected write.
    async fn probe(&self, request: &Request) -> Result<Response, Failure> {
        let response = self.endpoint.send(request).await?;
        if response.status == NOT_FOUND {
            return Err(self.endpoint.unsupported(&format!(
                "there is no /api/v{}; it may have been upgraded past this build",
                self.version
            )));
        }
        Ok(response)
    }
}

/// The status a service returns for a path its API version does not serve — here,
/// the whole versioned prefix, so it names an unsupported API version.
const NOT_FOUND: u16 = 404;

/// The fields of `system/status` that identify a service.
///
/// Named in camelCase as Servarr sends them; both name fields are optional
/// because they vary across the applications and their versions, and the one
/// that is present is used.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    #[serde(default)]
    instance_name: String,
    #[serde(default)]
    app_name: String,
    #[serde(default)]
    version: String,
}

#[async_trait]
impl Client for Servarr {
    async fn identity(&self) -> Result<Identity, Failure> {
        let response = self
            .probe(&self.request(Method::Get, "/system/status", None))
            .await?;
        let status: Status = self
            .endpoint
            .decode(&response, "the status response could not be read")?;
        let name = if status.instance_name.is_empty() {
            status.app_name
        } else {
            status.instance_name
        };
        if name.is_empty() || status.version.is_empty() {
            return Err(self
                .endpoint
                .refused("the service named neither itself nor its version"));
        }
        Ok(Identity {
            name,
            version: status.version,
        })
    }

    async fn register_download_client(&self, client: &DownloadClient) -> Result<(), Failure> {
        let response = self
            .probe(&self.request(
                Method::Post,
                "/downloadclient",
                Some(download_client_body(client)),
            ))
            .await?;
        self.endpoint.expect_success(&response)
    }

    async fn register_root_folder(&self, folder: &RootFolder) -> Result<(), Failure> {
        let body = serde_json::json!({ "path": folder.path }).to_string();
        let response = self
            .probe(&self.request(Method::Post, "/rootfolder", Some(body)))
            .await?;
        self.endpoint.expect_success(&response)
    }

    async fn root_folders(&self) -> Result<Vec<RegisteredFolder>, Failure> {
        let response = self
            .probe(&self.request(Method::Get, "/rootfolder", None))
            .await?;
        let folders: Vec<FolderResource> = self
            .endpoint
            .decode(&response, "the root-folder list could not be read")?;
        Ok(folders
            .into_iter()
            .map(|folder| RegisteredFolder {
                id: folder.id.to_string(),
                path: folder.path,
            })
            .collect())
    }

    async fn download_clients(&self) -> Result<Vec<RegisteredClient>, Failure> {
        let response = self
            .probe(&self.request(Method::Get, "/downloadclient", None))
            .await?;
        let clients: Vec<ClientResource> = self
            .endpoint
            .decode(&response, "the download-client list could not be read")?;
        Ok(clients
            .into_iter()
            .filter_map(ClientResource::endpoint)
            .collect())
    }
}

#[async_trait]
impl crate::ports::service::Maintenance for Servarr {
    async fn run_command(&self, name: &str) -> Result<(), Failure> {
        let body = serde_json::json!({ "name": name }).to_string();
        let response = self
            .probe(&self.request(Method::Post, "/command", Some(body)))
            .await?;
        self.endpoint.expect_success(&response)
    }
}

#[async_trait]
impl crate::ports::service::Queues for Servarr {
    async fn queue(&self) -> Result<QueueDepth, Failure> {
        // A generous page is asked for so the stuck count is read from the whole
        // queue rather than the default first page; the total is the service's own
        // count, independent of the page.
        let response = self
            .probe(&self.request(Method::Get, "/queue?pageSize=200", None))
            .await?;
        let queue: QueueResource = self
            .endpoint
            .decode(&response, "the queue could not be read")?;
        let stuck = queue
            .records
            .iter()
            .filter(|record| is_stuck(&record.tracked_download_status))
            .count();
        Ok(QueueDepth {
            total: queue.total_records,
            stuck,
        })
    }
}

/// Whether a queue item's tracked status is one that has stopped progressing —
/// the service's own words for a download that needs attention. Shared by the
/// dashboard's queue depth and a per-item trace.
fn is_stuck(status: &str) -> bool {
    status.eq_ignore_ascii_case("warning") || status.eq_ignore_ascii_case("error")
}

/// A page of the queue as the service reports it: its own total, and the records
/// on this page whose statuses the stuck count is read from.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueResource {
    #[serde(default)]
    total_records: usize,
    #[serde(default)]
    records: Vec<QueueRecord>,
}

/// One queued item — its tracked download status (for the dashboard's stuck count) and,
/// for a per-item trace, which item it belongs to and how far its download has got.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueRecord {
    #[serde(default)]
    tracked_download_status: String,
    #[serde(default)]
    tracked_download_state: String,
    #[serde(default)]
    series_id: Option<i64>,
    #[serde(default)]
    movie_id: Option<i64>,
}

impl QueueRecord {
    /// Whether this record is for the given item — matched on the id field the item's
    /// kind files under.
    fn is_for(&self, kind: crate::recyclarr::Kind, id: i64) -> bool {
        let owner = match kind {
            crate::recyclarr::Kind::Sonarr => self.series_id,
            crate::recyclarr::Kind::Radarr => self.movie_id,
        };
        owner == Some(id)
    }
}

/// A download-client resource as the service reports it: the identifier it
/// assigned, and the connection settings, which Servarr carries as named entries
/// in a `fields` array rather than as top-level keys.
#[derive(Deserialize)]
struct ClientResource {
    id: i64,
    #[serde(default)]
    fields: Vec<ClientField>,
}

/// One entry in a resource's `fields` array — a setting's name and its value,
/// whose type varies by setting, so it is read as untyped JSON and interpreted
/// per field.
#[derive(Deserialize)]
struct ClientField {
    name: String,
    #[serde(default)]
    value: serde_json::Value,
}

impl ClientResource {
    /// The client as the endpoint it reaches, or nothing where it does not name
    /// both a host and a port — which is all that a connection can be matched by,
    /// so one that names neither cannot be told from another and is left out.
    fn endpoint(self) -> Option<RegisteredClient> {
        let host = self.field("host")?.as_str()?.to_owned();
        let port = u16::try_from(self.field("port")?.as_u64()?).ok()?;
        Some(RegisteredClient {
            id: self.id.to_string(),
            host,
            port,
            category: self.category(),
        })
    }

    /// The category the client files under, read from whichever `*Category` field
    /// the target application names it — `tvCategory`, `movieCategory`,
    /// `musicCategory`. Nothing where the client carries no such field.
    fn category(&self) -> Option<crate::ports::service::Category> {
        let field = self
            .fields
            .iter()
            .find(|field| field.name.ends_with("Category"))?;
        Some(crate::ports::service::Category {
            field: field.name.clone(),
            value: field.value.as_str()?.to_owned(),
        })
    }

    /// The value of the named field, where the resource carries it.
    fn field(&self, name: &str) -> Option<&serde_json::Value> {
        self.fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| &field.value)
    }
}

/// The fields of a root-folder resource that seed reads back: the identifier the
/// service assigned, and the path, to match a wanted folder against.
#[derive(Deserialize)]
struct FolderResource {
    id: i64,
    path: String,
}

/// The API key a Servarr application wrote to its configuration, if it has
/// written one yet.
///
/// Each application generates its key on first start and records it in a small,
/// machine-generated XML file with the key in a single `<ApiKey>` element. It is
/// read as text rather than parsed as a tree: the format is fixed and exactly one
/// value is wanted, so a full XML dependency would be weight for nothing. An
/// absent or empty element reads as "not generated yet" — a service still
/// completing its first start, to be skipped and picked up on a later run — which
/// is `None`, never a fault.
#[must_use]
pub fn api_key(config_xml: &str) -> Option<String> {
    const OPEN: &str = "<ApiKey>";
    const CLOSE: &str = "</ApiKey>";
    let after_open = config_xml.find(OPEN)? + OPEN.len();
    let rest = config_xml.get(after_open..)?;
    let close = rest.find(CLOSE)?;
    let key = rest.get(..close)?.trim();
    (!key.is_empty()).then(|| key.to_owned())
}

/// The registration document for a download client, per the download-client
/// contract: the `implementation` and `configContract` that select the schema,
/// the protocol, and the `fields` array carrying the connection, the credential
/// the client uses, and the category the target application files under.
fn download_client_body(client: &DownloadClient) -> String {
    let (implementation, config_contract, protocol) = match client.kind {
        ClientKind::Sabnzbd => ("Sabnzbd", "SabnzbdSettings", "usenet"),
        ClientKind::Qbittorrent => ("QBittorrent", "QBittorrentSettings", "torrent"),
    };

    let mut fields = vec![
        serde_json::json!({ "name": "host", "value": client.host }),
        serde_json::json!({ "name": "port", "value": client.port }),
        serde_json::json!({ "name": client.category.field, "value": client.category.value }),
    ];
    match &client.credential {
        Credential::ApiKey(key) => {
            fields.push(serde_json::json!({ "name": "apiKey", "value": key }));
        }
        Credential::UserPass { username, password } => {
            fields.push(serde_json::json!({ "name": "username", "value": username }));
            fields.push(serde_json::json!({ "name": "password", "value": password }));
        }
    }

    serde_json::json!({
        "enable": true,
        "protocol": protocol,
        "name": client.name,
        "implementation": implementation,
        "configContract": config_contract,
        "fields": fields,
    })
    .to_string()
}
