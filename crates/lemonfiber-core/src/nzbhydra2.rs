//! Guarding the Usenet indexer aggregator's own configuration.
//!
//! `NZBHydra2` starts with no authentication, and in that state it answers a read of its
//! whole configuration — the indexer accounts it holds and their keys among it — to
//! anything that can reach it. Its API key guards only the searches made through it,
//! not the configuration. So lemonfiber turns authentication on, through the service's
//! own configuration API, and proves it by reading the configuration again presenting
//! nothing.
//!
//! **A change of authentication takes effect only once the service restarts.** It
//! restarts in place through its own control call, which has to be made while the
//! authentication it starts out with is still in force: once it restarts, the old way
//! in is gone.

use std::sync::Arc;

use base64::Engine as _;
use serde::Deserialize;
use serde_json::Value;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request, Response};
use crate::ports::service::Failure;

/// Where the service says how it is guarded, which it answers to anybody.
const ACCESS: &str = "/internalapi/userinfos";

/// The whole configuration, indexers and their keys among it.
const CONFIG: &str = "/internalapi/config";

/// The call that restarts the service in place.
const RESTART: &str = "/internalapi/control/restart";

/// The authentication lemonfiber turns on: a username and password on every call.
const BASIC: &str = "BASIC";

/// Every part of the service the authentication is to guard. Without at least one of
/// them the service refuses the change, saying the authentication would guard nothing.
const RESTRICTED: [&str; 5] = [
    "restrictAdmin",
    "restrictSearch",
    "restrictStats",
    "restrictDetailsDl",
    "restrictIndexerSelection",
];

/// What the service says of how it is guarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Access {
    /// Whether any authentication is configured.
    pub auth_configured: bool,
}

/// What the service says of a configuration it was handed.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    /// Whether it took it.
    ok: bool,
    /// Why not, in its own words.
    #[serde(default)]
    error_messages: Vec<String>,
}

/// Why a configuration handed to the service was not shown taken.
#[derive(Debug)]
pub enum Unguarded {
    /// Nothing changed: the configuration could not be guarded and was never sent, or the
    /// service said it did not take it.
    Untaken(Failure),
    /// Whether the service took it cannot be told: it was sent, and nothing that says
    /// came back.
    Unknown(Failure),
}

/// A username and a password, as the service's authentication takes them.
#[derive(Clone, Copy)]
pub struct Credential<'a> {
    /// The administrator's name.
    pub username: &'a str,
    /// The administrator's password.
    pub password: &'a str,
}

/// A client for one `NZBHydra2`.
pub struct Nzbhydra2 {
    endpoint: Endpoint,
}

impl Nzbhydra2 {
    /// A client for the service reached at `base`, named `service`.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>, service: impl Into<String>) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, service),
        }
    }

    /// A request to `path`, presenting `credential` where there is one.
    fn request(
        &self,
        method: Method,
        path: &str,
        credential: Option<Credential<'_>>,
        body: Option<String>,
    ) -> Request {
        let mut request = self.endpoint.json_request(method, path, body);
        if let Some(credential) = credential {
            let pair = format!("{}:{}", credential.username, credential.password);
            request.headers.push((
                "Authorization".to_owned(),
                format!(
                    "Basic {}",
                    base64::engine::general_purpose::STANDARD.encode(pair)
                ),
            ));
        }
        request
    }

    /// How the service says it is guarded.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the service is unreachable or answers unreadably.
    pub async fn access(&self) -> Result<Access, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, ACCESS, None, None))
            .await?;
        self.endpoint
            .decode(&response, "how the service is guarded could not be read")
    }

    /// The whole configuration, read presenting `credential` where there is one.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the service is unreachable, refuses what was presented,
    /// or answers unreadably.
    pub async fn config(&self, credential: Option<Credential<'_>>) -> Result<Value, Failure> {
        let response = self.read(credential).await?;
        self.endpoint
            .decode(&response, "the service's configuration could not be read")
    }

    /// Whether the service answers a read of its configuration to a caller presenting
    /// nothing.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the service is unreachable, or answers neither with the
    /// configuration nor with a refusal.
    pub async fn exposed(&self) -> Result<bool, Failure> {
        let response = self.read(None).await?;
        match response.status {
            401 | 403 => Ok(false),
            _ if response.is_success() => Ok(true),
            _ => Err(self.endpoint.refusal(&response)),
        }
    }

    /// The configuration read itself, whatever it answered.
    async fn read(&self, credential: Option<Credential<'_>>) -> Result<Response, Failure> {
        self.endpoint
            .send(&self.request(Method::Get, CONFIG, credential, None))
            .await
    }

    /// Hand the service `config` with authentication turned on for `admin` alone, every
    /// part of it guarded, and everything else in it exactly as it was.
    ///
    /// # Errors
    ///
    /// Returns [`Unguarded::Untaken`] where nothing changed — a configuration with no
    /// authentication section to turn on, or one the service refuses in its own words —
    /// and [`Unguarded::Unknown`] where it was sent and whether it was taken cannot be
    /// told.
    pub async fn guard(&self, mut config: Value, admin: Credential<'_>) -> Result<(), Unguarded> {
        let Some(auth) = config.get_mut("auth").and_then(Value::as_object_mut) else {
            return Err(Unguarded::Untaken(self.endpoint.refused(
                "the service's configuration holds no authentication section",
            )));
        };
        auth.insert("authType".to_owned(), Value::from(BASIC));
        for restricted in RESTRICTED {
            auth.insert(restricted.to_owned(), Value::Bool(true));
        }
        auth.insert(
            "users".to_owned(),
            serde_json::json!([{
                "username": admin.username,
                "password": admin.password,
                "maySeeAdmin": true,
                "maySeeStats": true,
                "maySeeDetailsDl": true,
                "showIndexerSelection": true,
            }]),
        );
        let response = self
            .endpoint
            .send(&self.request(Method::Put, CONFIG, None, Some(config.to_string())))
            .await
            .map_err(Unguarded::Unknown)?;
        if response.status >= 400 && response.status < 500 {
            return Err(Unguarded::Untaken(self.endpoint.refusal(&response)));
        }
        let saved: Saved = self
            .endpoint
            .decode(
                &response,
                "what the service made of the configuration could not be read",
            )
            .map_err(Unguarded::Unknown)?;
        if saved.ok {
            Ok(())
        } else {
            Err(Unguarded::Untaken(
                self.endpoint.refused(&saved.error_messages.join("; ")),
            ))
        }
    }

    /// Restart the service in place, which is when a change of authentication takes
    /// effect.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where the service is unreachable or refuses.
    pub async fn restart(&self) -> Result<(), Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, RESTART, None, None))
            .await?;
        self.endpoint.expect_success(&response)
    }
}

/// The name of every indexer a configuration holds, in the order it holds them, or
/// nothing where it holds no list of indexers to read them from.
#[must_use]
pub fn indexers(config: &Value) -> Option<Vec<String>> {
    config
        .get("indexers")
        .and_then(Value::as_array)
        .map(|held| {
            held.iter()
                .filter_map(|one| one.get("name").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
}

#[cfg(test)]
mod tests;
