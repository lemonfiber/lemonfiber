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

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::{
    Client, ClientKind, Credential, DownloadClient, Failure, Identity, RegisteredClient,
    RegisteredFolder, RootFolder,
};

/// The header every Servarr application authenticates with.
const API_KEY_HEADER: &str = "X-Api-Key";

/// A client for one Servarr-shape service.
///
/// Holds the endpoint — the transport, the service's address and its name — and
/// the key, which is the one thing the Servarr shape adds: a header on every
/// request.
pub struct Servarr {
    endpoint: Endpoint,
    key: String,
}

impl Servarr {
    /// A client for the service named `service`, reached at `base` with `key`.
    #[must_use]
    pub fn new(
        http: Arc<dyn Http>,
        base: impl Into<String>,
        key: impl Into<String>,
        service: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, service),
            key: key.into(),
        }
    }

    /// A request to a path under the service's versioned API, carrying the key.
    fn request(&self, method: Method, path: &str, body: Option<String>) -> Request {
        Request {
            method,
            url: self.endpoint.url(&format!("/api/v3{path}")),
            headers: vec![(API_KEY_HEADER.to_owned(), self.key.clone())],
            body,
        }
    }
}

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
            .endpoint
            .send(&self.request(Method::Get, "/system/status", None))
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
            .endpoint
            .send(&self.request(
                Method::Post,
                "/downloadclient",
                Some(download_client_body(client)),
            ))
            .await?;
        if response.is_success() {
            Ok(())
        } else {
            Err(self.endpoint.refusal(&response))
        }
    }

    async fn register_root_folder(&self, folder: &RootFolder) -> Result<(), Failure> {
        let body = serde_json::json!({ "path": folder.path }).to_string();
        let response = self
            .endpoint
            .send(&self.request(Method::Post, "/rootfolder", Some(body)))
            .await?;
        if response.is_success() {
            Ok(())
        } else {
            Err(self.endpoint.refusal(&response))
        }
    }

    async fn root_folders(&self) -> Result<Vec<RegisteredFolder>, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, "/rootfolder", None))
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
            .endpoint
            .send(&self.request(Method::Get, "/downloadclient", None))
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
