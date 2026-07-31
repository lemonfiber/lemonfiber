//! Reading `SABnzbd` — its configuration, and its queue for the dashboard.
//!
//! `SABnzbd` is a download client the Servarr apps are told about, not a service
//! lemonfiber sends wiring commands to, so unlike the Servarr shape seed sends it
//! nothing — it only reads the one value that registers it: the API key `SABnzbd`
//! generates for itself on first start. The dashboard does ask it one thing,
//! though — what it is downloading right now — so a small read-only client lives
//! here too, alongside the key reader.
//!
//! The key lives in `sabnzbd.ini`, a plain INI file, under a single `api_key`
//! entry. It is read as text rather than through an INI dependency: one known key
//! is wanted from a fixed format, so a parser would be weight for nothing — the
//! same reasoning as the Servarr key reader ([`crate::servarr::api_key`]). An
//! absent or empty entry reads as "not generated yet" — a service still
//! completing its first start, to be skipped and picked up on a later run — which
//! is `None`, never a fault.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::{Download, Failure, Transfers};

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

/// `SABnzbd`'s `mode=queue` answer: the queue, and inside it the slots and the one
/// speed the whole queue downloads at.
#[derive(Deserialize)]
struct QueueResponse {
    queue: Queue,
}

/// The queue as `SABnzbd` reports it. `kbpersec` is the download rate for the
/// whole queue — `SABnzbd` downloads one item at a time — so it belongs to
/// whichever slot is the active one.
#[derive(Deserialize)]
struct Queue {
    #[serde(default)]
    kbpersec: String,
    #[serde(default)]
    slots: Vec<Slot>,
}

/// One queued download as `SABnzbd` reports it, every figure a string.
#[derive(Deserialize)]
struct Slot {
    filename: String,
    percentage: String,
    status: String,
    timeleft: String,
    #[serde(default)]
    mbleft: String,
}

#[async_trait]
impl Transfers for Sabnzbd {
    async fn transfers(&self) -> Result<Vec<Download>, Failure> {
        let request = Request {
            method: Method::Get,
            url: self
                .endpoint
                .url(&format!("/api?mode=queue&output=json&apikey={}", self.key)),
            headers: Vec::new(),
            body: None,
        };
        let response = self.endpoint.send(&request).await?;
        let body: QueueResponse = self
            .endpoint
            .decode(&response, "the queue could not be read")?;
        let active_speed = bytes_per_second(&body.queue.kbpersec);
        Ok(body
            .queue
            .slots
            .into_iter()
            .map(|slot| download_of(slot, active_speed))
            .collect())
    }
}

/// One slot as the dashboard's [`Download`]. The queue's single speed is the
/// active slot's; every other slot is not moving, so it reads a definite zero
/// rather than the queue's rate or an unknown.
fn download_of(slot: Slot, active_speed: Option<u64>) -> Download {
    let speed = if slot.status == DOWNLOADING {
        active_speed
    } else {
        Some(0)
    };
    Download {
        name: slot.filename,
        progress: slot.percentage.trim().parse::<u8>().unwrap_or(0).min(100),
        speed,
        eta: seconds_left(&slot.timeleft)
            .filter(|left| *left > 0)
            .map(Duration::from_secs),
        remaining: bytes_left(&slot.mbleft),
    }
}

/// `SABnzbd`'s `mbleft` — a decimal string of megabytes still to fetch — as bytes,
/// or `None` where it will not parse. The whole-megabyte part is taken, matching
/// [`bytes_per_second`]'s reasoning: sub-megabyte precision is far below the
/// gigabyte scale the free-space projection weighs, so carrying the fraction would
/// need a float cast for no difference the operator could see.
fn bytes_left(mbleft: &str) -> Option<u64> {
    let whole = mbleft.split_once('.').map_or(mbleft, |(whole, _)| whole);
    whole
        .trim()
        .parse::<u64>()
        .ok()
        .map(|mb| mb.saturating_mul(1024 * 1024))
}

/// `SABnzbd`'s `kbpersec` — a decimal string of kilobytes per second — as bytes
/// per second, or `None` where it will not parse. The whole-kilobyte part is
/// taken; sub-kilobyte precision is below anything the dashboard shows, so
/// carrying the fraction would need a float cast for no visible difference.
fn bytes_per_second(kbpersec: &str) -> Option<u64> {
    let whole = kbpersec
        .split_once('.')
        .map_or(kbpersec, |(whole, _)| whole);
    whole
        .trim()
        .parse::<u64>()
        .ok()
        .map(|kb| kb.saturating_mul(1024))
}

/// A `SABnzbd` `timeleft` — colon-separated `H:MM:SS`, the hours unbounded — as
/// whole seconds, or `None` where any field will not parse. Every field is base
/// sixty, so folding by sixty is exact however many there are; an empty string is
/// `None` rather than zero.
fn seconds_left(timeleft: &str) -> Option<u64> {
    if timeleft.is_empty() {
        return None;
    }
    let mut total: u64 = 0;
    for field in timeleft.split(':') {
        let value: u64 = field.trim().parse().ok()?;
        total = total.saturating_mul(60).saturating_add(value);
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::api_key;

    /// A minimal `sabnzbd.ini` as `SABnzbd` writes it, with the key under `[misc]`
    /// alongside a similarly-named entry the reader must not mistake for it.
    const CONFIG: &str = "\
[misc]
host = 0.0.0.0
api_key = the-key
nzb_key = ffffffffffff
";

    #[test]
    fn the_generated_key_is_read_from_its_entry() {
        assert_eq!(api_key(CONFIG).as_deref(), Some("the-key"));
    }

    #[test]
    fn a_neighbouring_key_entry_is_not_mistaken_for_it() {
        // `nzb_key` shares the suffix but is a different value; only `api_key`
        // is the download client's credential.
        let only_nzb = "[misc]\nnzb_key = ffffffffffff\n";
        assert_eq!(api_key(only_nzb), None);
    }

    #[test]
    fn a_commented_entry_is_not_read_as_the_key() {
        // A commented-out line keeps the `#` on the name, so it does not match.
        assert_eq!(api_key("#api_key = the-key"), None);
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert_eq!(api_key("api_key =   the-key  ").as_deref(), Some("the-key"));
    }

    #[test]
    fn a_key_not_generated_yet_is_absent_not_a_fault() {
        // Present but empty until first start completes.
        assert_eq!(api_key("[misc]\napi_key =\n"), None);
        // Only whitespace after the separator is also not-yet.
        assert_eq!(api_key("api_key =    "), None);
        // Or the entry is not there at all yet.
        assert_eq!(api_key("[misc]\nhost = 0.0.0.0\n"), None);
    }

    #[test]
    fn a_section_header_is_not_read_as_a_key() {
        // A line with no separator, such as a section header, is passed over.
        assert_eq!(api_key("[misc]"), None);
    }

    #[test]
    fn a_multibyte_value_survives_intact() {
        assert_eq!(api_key("api_key = café☃clé").as_deref(), Some("café☃clé"));
    }
}

#[cfg(test)]
mod transfers_tests {
    use std::sync::Arc;
    use std::time::Duration;

    use crate::ports::service::{Failure, Transfers};
    use crate::test_support::ScriptedHttp;

    use super::Sabnzbd;

    /// A client whose transport answers the queue call from `replies`.
    fn client(replies: Vec<(u16, &'static str)>) -> Sabnzbd {
        Sabnzbd::new(
            Arc::new(ScriptedHttp::new(replies)),
            "http://127.0.0.1:8080",
            "key",
        )
    }

    /// One slot downloading with a real speed and countdown, one queued behind it.
    /// The active slot's `mbleft` carries a fraction (so the whole-megabyte path is
    /// exercised); the waiting slot's is a bare integer.
    const QUEUE: &str = r#"{"queue":{"kbpersec":"2048.5","slots":[
        {"filename":"Active.nzb","percentage":"45","status":"Downloading","timeleft":"0:10:00","mbleft":"1024.5"},
        {"filename":"Waiting.nzb","percentage":"0","status":"Queued","timeleft":"0:00:00","mbleft":"512"}
    ]}}"#;

    /// Edge values: an unreadable queue speed, a percentage that will not parse and
    /// one over a hundred, an empty and a malformed countdown, a paused slot.
    const QUEUE_EDGES: &str = r#"{"queue":{"kbpersec":"nan","slots":[
        {"filename":"BadSpeed.nzb","percentage":"oops","status":"Downloading","timeleft":"","mbleft":"oops"},
        {"filename":"Paused.nzb","percentage":"200","status":"Paused","timeleft":"1:bad:3"}
    ]}}"#;

    #[tokio::test]
    async fn the_active_slot_carries_the_queue_speed_and_the_rest_read_zero() {
        let sab = client(vec![(200, QUEUE)]);
        let transfers = sab.transfers().await.unwrap_or_default();
        assert_eq!(transfers.len(), 2);
        assert!(matches!(
            transfers.first(),
            Some(t) if t.name == "Active.nzb"
                && t.progress == 45
                && t.speed == Some(2048 * 1024)
                && t.eta == Some(Duration::from_secs(600))
                && t.remaining == Some(1024 * 1024 * 1024)
        ));
        // A queued slot is not moving: a definite zero, and no estimate to give —
        // but its bytes still to fetch count towards what the queue is committed to.
        assert!(matches!(
            transfers.get(1),
            Some(t) if t.progress == 0 && t.speed == Some(0) && t.eta.is_none()
                && t.remaining == Some(512 * 1024 * 1024)
        ));
    }

    #[tokio::test]
    async fn unreadable_figures_degrade_rather_than_fail_the_read() {
        let sab = client(vec![(200, QUEUE_EDGES)]);
        let transfers = sab.transfers().await.unwrap_or_default();
        assert_eq!(transfers.len(), 2);
        // An unparsable speed is unknown, an unparsable percentage is zero, an
        // empty countdown is no estimate, and an unparsable `mbleft` is no figure.
        assert!(matches!(
            transfers.first(),
            Some(t) if t.progress == 0 && t.speed.is_none() && t.eta.is_none()
                && t.remaining.is_none()
        ));
        // A percentage over a hundred is clamped; a malformed field is no estimate.
        // An absent `mbleft` reads the same as an unparsable one: no figure.
        assert!(matches!(
            transfers.get(1),
            Some(t) if t.progress == 100 && t.speed == Some(0) && t.eta.is_none()
                && t.remaining.is_none()
        ));
    }

    #[tokio::test]
    async fn a_client_that_will_not_answer_is_unavailable() {
        let sab = client(Vec::new());
        assert!(matches!(
            sab.transfers().await,
            Err(Failure::Unavailable { .. })
        ));
    }

    #[tokio::test]
    async fn a_queue_that_will_not_parse_is_refused() {
        let sab = client(vec![(200, "not json")]);
        assert!(matches!(
            sab.transfers().await,
            Err(Failure::Refused { .. })
        ));
    }
}
