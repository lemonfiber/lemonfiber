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
    Client, ClientKind, ClientProbe, Credential, DownloadClient, Failure, Identity, QualityProfile,
    RegisteredClient, RegisteredFolder, RootFolder,
};

mod catalogue;
mod importing;
mod pipeline;
mod wire;

// The shapes the service sends, read by the wiring here and by the queue and
// trace beside it — one idea of the JSON rather than three.
use wire::{ClientResource, FolderResource, QueueRecord, QueueResource, TestResource};
mod quality;
mod queue;

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
                Some(download_client_body(client, None)),
            ))
            .await?;
        self.endpoint.expect_success(&response)
    }

    async fn update_download_client(
        &self,
        id: &str,
        client: &DownloadClient,
    ) -> Result<(), Failure> {
        // Servarr updates a client with a PUT to its own id, carrying the same
        // registration document a create does but with the id set, so it rewrites the
        // one that is there rather than adding a second. An id the service did not
        // assign as an integer is one this cannot address, so it is refused rather than
        // guessed at.
        let Ok(numeric) = id.parse::<i64>() else {
            return Err(self
                .endpoint
                .refused("the download client's id is not one this service assigns"));
        };
        let response = self
            .probe(&self.request(
                Method::Put,
                &format!("/downloadclient/{id}"),
                Some(download_client_body(client, Some(numeric))),
            ))
            .await?;
        self.endpoint.expect_success(&response)
    }

    async fn set_client_field(
        &self,
        id: &str,
        field: &str,
        value: Option<&str>,
    ) -> Result<(), Failure> {
        // Read, change the one field, write back. Servarr takes a whole resource document
        // on a PUT, so putting one field back means sending the rest of the document
        // exactly as the service gave it — which is also what keeps a reversal from having
        // to know the client's credential to restore its category.
        let response = self
            .probe(&self.request(Method::Get, &format!("/downloadclient/{id}"), None))
            .await?;
        let mut document: serde_json::Value = self
            .endpoint
            .decode(&response, "the download client could not be read")?;
        let Some(fields) = document
            .get_mut("fields")
            .and_then(serde_json::Value::as_array_mut)
        else {
            return Err(self
                .endpoint
                .refused("the download client has no settings to put back"));
        };
        set_field(fields, field, value);
        let response = self
            .probe(&self.request(
                Method::Put,
                &format!("/downloadclient/{id}"),
                Some(document.to_string()),
            ))
            .await?;
        self.endpoint.expect_success(&response)
    }

    async fn test_download_clients(&self) -> Result<Vec<ClientProbe>, Failure> {
        // Servarr tests every configured client at once with a POST to `testall`,
        // answering with one result per client: its id and whether it validated,
        // with the failure messages where it did not. A client that failed the test
        // is not an error — it is the very answer wanted — so only a service that
        // will not run the test at all is a `Failure`.
        let response = self
            .probe(&self.request(Method::Post, "/downloadclient/testall", None))
            .await?;
        let results: Vec<TestResource> = self.endpoint.decode(
            &response,
            "the download-client test results could not be read",
        )?;
        Ok(results.into_iter().map(TestResource::probe).collect())
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

    async fn quality_profiles(&self) -> Result<Vec<QualityProfile>, Failure> {
        let held: Vec<crate::servarr::catalogue::ProfileResource> =
            self.read("/qualityprofile", "the quality profiles").await?;
        // A profile with no usable id is one nothing could be fetched at, and a
        // nameless one is one the request service would show a blank beside. Both
        // are passed over rather than carried as a half-answer.
        Ok(held
            .into_iter()
            .filter_map(|profile| {
                u32::try_from(profile.id)
                    .ok()
                    .filter(|_| !profile.name.is_empty())
                    .map(|id| QualityProfile {
                        id,
                        name: profile.name,
                    })
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
/// Put one named field of a resource's `fields` array to a value, adding it where the
/// resource does not carry it and taking it out where it is being cleared.
///
/// Servarr carries a resource's settings as a list of name/value pairs rather than as
/// object keys, so changing one is a search rather than an assignment.
fn set_field(fields: &mut Vec<serde_json::Value>, field: &str, value: Option<&str>) {
    let Some(value) = value else {
        fields.retain(|held| held.get("name").and_then(serde_json::Value::as_str) != Some(field));
        return;
    };
    if let Some(held) = fields
        .iter_mut()
        .find(|held| held.get("name").and_then(serde_json::Value::as_str) == Some(field))
    {
        held["value"] = serde_json::Value::String(value.to_owned());
        return;
    }
    fields.push(serde_json::json!({ "name": field, "value": value }));
}

fn download_client_body(client: &DownloadClient, id: Option<i64>) -> String {
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

    let mut document = serde_json::json!({
        "enable": true,
        "protocol": protocol,
        "name": client.name,
        "implementation": implementation,
        "configContract": config_contract,
        "fields": fields,
    });
    // An update names the client the service already assigned, so the same document
    // rewrites it in place; a create carries no id and the service assigns one.
    if let (Some(id), Some(object)) = (id, document.as_object_mut()) {
        object.insert("id".to_owned(), serde_json::json!(id));
    }
    document.to_string()
}
