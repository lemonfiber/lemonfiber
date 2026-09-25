//! Reading `SABnzbd` — its configuration, its queue, and the accounts behind it.
//!
//! `SABnzbd` is a download client the Servarr apps are told about, not a service
//! lemonfiber sends wiring commands to, so unlike the Servarr shape seed sends it
//! nothing — it only reads the one value that registers it: the API key `SABnzbd`
//! generates for itself on first start. The dashboard does ask it one thing,
//! though — what it is downloading right now — so a small read-only client lives
//! here too, alongside the key reader.
//!
//! It is also the only place the Usenet accounts are legible at all: a provider
//! publishes no quota, so the block that was bought is recorded in the client and the
//! bytes pulled are measured there. Reading those is the third thing this asks for,
//! and the client's own dialect — sizes stepped by 1024, booleans written as 0 and 1,
//! counters that reset on the calendar — is translated here rather than leaking out.
//!
//! The key lives in `sabnzbd.ini`, a plain INI file, under a single `api_key`
//! entry. It is read as text rather than through an INI dependency: one known key
//! is wanted from a fixed format, so a parser would be weight for nothing — the
//! same reasoning as the Servarr key reader ([`crate::servarr::api_key`]). An
//! absent or empty entry reads as "not generated yet" — a service still
//! completing its first start, to be skipped and picked up on a later run — which
//! is `None`, never a fault.

use std::sync::Arc;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::Failure;

/// The service name a failure is reported against.
const SERVICE: &str = "sabnzbd";

/// The `status` a slot carries while it is the one being downloaded — the active
/// slot, to which the queue's single speed belongs.
const DOWNLOADING: &str = "Downloading";

/// The API key `SABnzbd` wrote to its configuration, if it has written one yet.
///
/// The `api_key` entry is matched by its exact name so a neighbouring `nzb_key`
/// or a `#`-commented line is not read as the key. An entry that is present but
/// empty is a first start not yet finished, and is `None` like an absent one.
#[must_use]
pub fn api_key(config_ini: &str) -> Option<String> {
    config_ini.lines().find_map(read_api_key)
}

/// One line as the API key it sets, where it is the `api_key` entry with a value.
fn read_api_key(line: &str) -> Option<String> {
    let (name, value) = line.split_once('=')?;
    if name.trim() != "api_key" {
        return None;
    }
    let key = value.trim();
    (!key.is_empty()).then(|| key.to_owned())
}

/// A read-only client for one `SABnzbd`, for the dashboard's transfers panel.
pub struct Sabnzbd {
    endpoint: Endpoint,
    key: String,
}

impl Sabnzbd {
    /// A client for the `SABnzbd` reached at `base`, authenticating with `key`.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, SERVICE),
            key: key.into(),
        }
    }
}

impl Sabnzbd {
    /// One `mode=` call, decoded — the shape every read here shares.
    async fn read<T: serde::de::DeserializeOwned>(
        &self,
        mode: &str,
        whenever: &str,
    ) -> Result<T, Failure> {
        let request = Request {
            method: Method::Get,
            url: self
                .endpoint
                .url(&format!("/api?mode={mode}&output=json&apikey={}", self.key)),
            headers: Vec::new(),
            body: None,
        };
        let response = self.endpoint.send(&request).await?;
        self.endpoint.decode(&response, whenever)
    }

    /// One call to a configuration page, which is where the scheduler is written.
    ///
    /// Apart from the `mode=` API because it is a different door: the schedule is
    /// the one setting this client will not take through the API without damage —
    /// see [`scheduling`] — and its own pages take a change one line at a time and
    /// reload the running scheduler afterwards, which the API door does not.
    ///
    /// The page answers a write with a redirect to the page it was reached from,
    /// and nothing behind that redirect says what became of the request. So this
    /// reads the status and no more; every caller settles what happened by reading
    /// the schedule back.
    async fn page(&self, path: &str, fields: &[(&str, &str)]) -> Result<(), Failure> {
        let mut fields = fields.to_vec();
        fields.push(("apikey", self.key.as_str()));
        let request = Request {
            method: Method::Get,
            url: self.endpoint.url(&format!(
                "/config/{path}?{}",
                crate::endpoint::form_encoded(&fields)
            )),
            headers: Vec::new(),
            body: None,
        };
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)
    }
}

/// `SABnzbd`'s answer to a write.
///
/// The client answers a setting it would not take with a `false` here and a `200`
/// around it, so the status is read rather than the request being called done
/// because it arrived.
#[derive(serde::Deserialize)]
struct Wrote {
    #[serde(default)]
    status: bool,
}

mod accounts;
mod fetching;
mod metering;
mod queue;
mod scheduling;
mod throttling;

#[cfg(test)]
mod tests;
