//! Telling the subtitle finder which \*arrs to watch.
//!
//! Bazarr does not discover them. Until it is told, it runs with nothing to look at
//! — the household gets subtitles for nothing, which is indistinguishable from a
//! household whose releases happen not to have any.
//!
//! **Its settings are written as a form, not as JSON**, and its field names are the
//! configuration file's own path flattened: `settings-<section>-<field>`. So
//! pointing it at Sonarr is `settings-sonarr-ip` and the rest, beside the
//! `settings-general-use_sonarr` that decides whether any of it is read.
//!
//! **The write is partial.** Sending one \*arr's fields leaves the other's exactly
//! as they were, which is what lets an \*arr that is not running be skipped and
//! completed on a later pass, the way every other connection behaves.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::endpoint::{form_content_type, form_encoded, Endpoint};
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::{Failure, Subtitled, Subtitles, Watched, Watching};

/// The header the key is presented in.
const KEY: &str = "X-API-KEY";

/// Where the whole of the settings are read and written.
const SETTINGS: &str = "/api/system/settings";

/// What one \*arr's settings hold, as Bazarr reports them.
///
/// The base path is deliberately not read: it is normalised on the way in — `/` is
/// stored as empty — so carrying it would mean comparing a value against a
/// different spelling of itself.
#[derive(Deserialize)]
struct ArrSettings {
    #[serde(default)]
    ip: String,
    #[serde(default)]
    port: u16,
    #[serde(default)]
    apikey: String,
}

/// The one general field this reads: whether an \*arr is used at all.
#[derive(Deserialize, Default)]
struct General {
    #[serde(default)]
    use_sonarr: bool,
    #[serde(default)]
    use_radarr: bool,
}

/// The settings document, of which this reads three parts.
#[derive(Deserialize)]
struct Settings {
    #[serde(default)]
    general: General,
    #[serde(default)]
    sonarr: Option<ArrSettings>,
    #[serde(default)]
    radarr: Option<ArrSettings>,
}

/// Bazarr's own key, read from the configuration it writes.
///
/// **The name is not enough to find it.** The file holds an `apikey` under `auth`,
/// and another under each \*arr it has been pointed at. Its sections are written in
/// alphabetical order, so `auth` leads both today and a scan for the first `apikey`
/// happens to land on the right one — by an accident of spelling rather than because
/// the name identifies anything. What identifies it is the section it sits under, and
/// a provider section sorting before `auth` with a field of that name would be read
/// as this service's key.
///
/// So the section is tracked: a key in the first column opens one, and only the
/// `apikey` inside `auth` is this service's. Scanned rather than parsed because
/// nothing else in the file is wanted, and a parser would be a dependency taken on
/// for one line.
#[must_use]
pub fn api_key(config_yaml: &str) -> Option<String> {
    let mut section = None;
    for line in config_yaml.lines() {
        if let Some(opened) = section_key(line) {
            section = Some(opened);
            continue;
        }
        if section == Some(AUTH) {
            if let Some(key) = read_api_key(line) {
                return Some(key);
            }
        }
    }
    None
}

/// The section Bazarr keeps its own credential in.
const AUTH: &str = "auth";

/// The section `line` opens, where it is a key in the first column.
fn section_key(line: &str) -> Option<&str> {
    let first = line.chars().next()?;
    if first.is_whitespace() || first == '#' {
        return None;
    }
    Some(line.split_once(':')?.0.trim())
}

/// One line as the key it sets, where it is the `apikey` entry with a value.
fn read_api_key(line: &str) -> Option<String> {
    let (name, value) = line.split_once(':')?;
    if name.trim() != "apikey" {
        return None;
    }
    let key = value.trim().trim_matches('\'').trim_matches('"');
    (!key.is_empty()).then(|| key.to_owned())
}

/// A client for one Bazarr.
pub struct Bazarr {
    endpoint: Endpoint,
    /// The key it presents, read from Bazarr's own configuration.
    key: String,
}

impl Bazarr {
    /// A client for the Bazarr reached at `base`, named `service`, holding `key`.
    #[must_use]
    pub fn new(
        http: Arc<dyn Http>,
        base: impl Into<String>,
        service: impl Into<String>,
        key: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, service),
            key: key.into(),
        }
    }

    /// A request carrying the key, and a form body where there is one.
    fn request(&self, method: Method, body: Option<String>) -> Request {
        let mut headers = vec![(KEY.to_owned(), self.key.clone())];
        if body.is_some() {
            headers.push(form_content_type());
        }
        Request {
            method,
            url: self.endpoint.url(SETTINGS),
            headers,
            body,
        }
    }
}

/// The fields as Bazarr names them: the configuration path, flattened.
fn field(section: &str, name: &str) -> String {
    format!("settings-{section}-{name}")
}

#[async_trait]
impl Subtitles for Bazarr {
    async fn watching(&self, which: Subtitled) -> Result<Watching, Failure> {
        let response = self.endpoint.send(&self.request(Method::Get, None)).await?;
        let held: Settings = self.endpoint.decode(
            &response,
            "the subtitle finder's settings could not be read",
        )?;
        let (enabled, arr) = match which {
            Subtitled::Sonarr => (held.general.use_sonarr, held.sonarr),
            Subtitled::Radarr => (held.general.use_radarr, held.radarr),
        };
        let arr = arr.unwrap_or(ArrSettings {
            ip: String::new(),
            port: 0,
            apikey: String::new(),
        });
        Ok(Watching {
            enabled,
            host: arr.ip,
            port: arr.port,
            keyed: !arr.apikey.is_empty(),
        })
    }

    async fn watch(&self, watched: &Watched) -> Result<(), Failure> {
        let section = watched.which.section();
        let port = watched.port.to_string();
        let used = field("general", &format!("use_{section}"));
        let body = form_encoded(&[
            // The switch and the address together: either alone is nothing. An
            // address it is not set to use is never read, and switching it on with
            // no address gives it somewhere unreachable to look.
            (used.as_str(), "true"),
            (&field(section, "ip"), &watched.host),
            (&field(section, "port"), &port),
            (&field(section, "apikey"), &watched.api_key),
        ]);
        let written = self
            .endpoint
            .send(&self.request(Method::Post, Some(body)))
            .await?;
        self.endpoint.expect_success(&written)
    }
}
