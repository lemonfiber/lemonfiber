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

use crate::ports::http::{Http, Method, Request, Response};
use crate::ports::service::{Client, DownloadClient, Failure, Identity, RootFolder};

/// The header every Servarr application authenticates with.
const API_KEY_HEADER: &str = "X-Api-Key";

/// A client for one Servarr-shape service.
///
/// Holds the transport, the service's address and key, and its name — the last
/// only so a failure can say which service it is about.
pub struct Servarr {
    http: Arc<dyn Http>,
    base: String,
    key: String,
    service: String,
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
            http,
            base: base.into(),
            key: key.into(),
            service: service.into(),
        }
    }

    /// A request to a path under the service's versioned API, carrying the key.
    fn request(&self, method: Method, path: &str, body: Option<String>) -> Request {
        Request {
            method,
            url: format!("{}/api/v3{path}", self.base.trim_end_matches('/')),
            headers: vec![(API_KEY_HEADER.to_owned(), self.key.clone())],
            body,
        }
    }

    /// Send a request, turning a transport failure into "not answering".
    ///
    /// The port reports the no-answer case; here it becomes the service failure
    /// the rest of the system acts on — a prerequisite that is not up yet, to be
    /// skipped and picked up on a later run rather than counted as broken.
    async fn send(&self, request: &Request) -> Result<Response, Failure> {
        let result = self.http.send(request).await;
        result.map_err(|_| Failure::Unavailable {
            service: self.service.clone(),
        })
    }

    /// What a non-success response amounts to: a refused credential, or an answer
    /// lemonfiber cannot use, carrying the service's own words.
    fn refusal(&self, response: &Response) -> Failure {
        match response.status {
            401 | 403 => Failure::Unauthorised {
                service: self.service.clone(),
            },
            _ => Failure::Refused {
                service: self.service.clone(),
                detail: describe(response),
            },
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
            .send(&self.request(Method::Get, "/system/status", None))
            .await?;
        if !response.is_success() {
            return Err(self.refusal(&response));
        }
        let status: Status =
            serde_json::from_str(&response.body).map_err(|_| Failure::Refused {
                service: self.service.clone(),
                detail: "the status response could not be read".to_owned(),
            })?;
        let name = if status.instance_name.is_empty() {
            status.app_name
        } else {
            status.instance_name
        };
        if name.is_empty() || status.version.is_empty() {
            return Err(Failure::Refused {
                service: self.service.clone(),
                detail: "the service named neither itself nor its version".to_owned(),
            });
        }
        Ok(Identity {
            name,
            version: status.version,
        })
    }

    async fn register_download_client(&self, client: &DownloadClient) -> Result<(), Failure> {
        // The full per-implementation schema — the field set differs between
        // SABnzbd and qBittorrent — is filled in when seed wires a concrete
        // client and the port carries which one it is. This registers the
        // connection lemonfiber knows about; the endpoint and auth are settled.
        let body = serde_json::json!({
            "enable": true,
            "name": client.name,
            "host": client.host,
            "port": client.port,
        })
        .to_string();
        let response = self
            .send(&self.request(Method::Post, "/downloadclient", Some(body)))
            .await?;
        if response.is_success() {
            Ok(())
        } else {
            Err(self.refusal(&response))
        }
    }

    async fn register_root_folder(&self, folder: &RootFolder) -> Result<(), Failure> {
        let body = serde_json::json!({ "path": folder.path }).to_string();
        let response = self
            .send(&self.request(Method::Post, "/rootfolder", Some(body)))
            .await?;
        if response.is_success() {
            Ok(())
        } else {
            Err(self.refusal(&response))
        }
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

/// A non-success response as the detail of a refusal: the service's own words
/// where it gave any, its status code alone otherwise.
fn describe(response: &Response) -> String {
    let body = response.body.trim();
    if body.is_empty() {
        format!("HTTP {}", response.status)
    } else {
        format!("HTTP {}: {body}", response.status)
    }
}
