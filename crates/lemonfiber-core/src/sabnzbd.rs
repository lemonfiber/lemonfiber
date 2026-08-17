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
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use lemonfiber_manifest::Date;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::{
    Download, Failure, Recorded, Transfers, UsenetAccount, UsenetAccounts,
};

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
}

#[async_trait]
impl Transfers for Sabnzbd {
    async fn transfers(&self) -> Result<Vec<Download>, Failure> {
        let body: QueueResponse = self.read("queue", "the queue could not be read").await?;
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

/// `SABnzbd`'s `mode=get_config&section=servers` answer: the accounts as configured.
#[derive(Deserialize)]
struct ConfigResponse {
    config: Servers,
}

#[derive(Deserialize)]
struct Servers {
    #[serde(default)]
    servers: Vec<ServerConfig>,
}

/// One configured account. The username and the starred-out password come back in
/// this answer too and are deliberately not read: nothing here needs them, and a
/// field that is never taken cannot be logged, reported, or put in a bundle.
#[derive(Deserialize)]
struct ServerConfig {
    /// The account's own key, which is also how the statistics are keyed.
    name: String,
    /// What the operator named it, where they named it anything.
    #[serde(default)]
    displayname: String,
    /// `SABnzbd` writes its booleans as 0 or 1. A missing flag reads as in use:
    /// over-reporting an account is a smaller error than silently dropping one.
    #[serde(default = "in_use")]
    enable: i64,
    /// The allowance, in the client's own size notation.
    #[serde(default)]
    quota: String,
    /// What had been pulled when that allowance was recorded.
    #[serde(default)]
    usage_at_start: i64,
    /// The subscription's last day, as an ISO date.
    #[serde(default)]
    expire_date: String,
}

/// The default for a missing enabled flag — see [`ServerConfig::enable`].
const fn in_use() -> i64 {
    1
}

/// `SABnzbd`'s `mode=server_stats` answer: what each account has pulled, keyed by
/// the same name the configuration uses.
#[derive(Deserialize)]
struct StatsResponse {
    #[serde(default)]
    servers: std::collections::BTreeMap<String, ServerStats>,
}

#[derive(Deserialize)]
struct ServerStats {
    /// Everything ever pulled from the account.
    #[serde(default)]
    total: u64,
    /// Bytes per day, keyed by `YYYY-MM-DD`. The client's own week and month totals
    /// are deliberately ignored: they reset on the calendar rather than rolling, so a
    /// rate taken from them depends on what day it happens to be.
    #[serde(default)]
    daily: std::collections::BTreeMap<String, u64>,
}

#[async_trait]
impl UsenetAccounts for Sabnzbd {
    async fn accounts(&self) -> Result<Vec<UsenetAccount>, Failure> {
        let configured: ConfigResponse = self
            .read(
                "get_config&section=servers",
                "the Usenet accounts could not be read",
            )
            .await?;
        let measured: StatsResponse = self
            .read("server_stats", "the account statistics could not be read")
            .await?;
        Ok(configured
            .config
            .servers
            .into_iter()
            .map(|server| {
                let stats = measured.servers.get(&server.name);
                account_of(server, stats)
            })
            .collect())
    }
}

/// One configured account joined to what it has pulled.
///
/// An account the statistics have never mentioned has downloaded nothing yet, which
/// is a zero rather than a gap: the client would not measure an account it has not
/// used, and reporting that as unreadable would raise a fault about an idle provider.
fn account_of(server: ServerConfig, stats: Option<&ServerStats>) -> UsenetAccount {
    let downloaded = stats.map_or(0, |stats| stats.total);
    let name = if server.displayname.trim().is_empty() {
        server.name
    } else {
        server.displayname
    };
    UsenetAccount {
        name,
        enabled: server.enable != 0,
        quota: recorded_quota(&server.quota, server.usage_at_start),
        downloaded,
        daily: stats.map(daily_totals).unwrap_or_default(),
        expires_on: Date::parse(server.expire_date.trim()),
    }
}

/// The allowance recorded against an account, where one is recorded at all.
///
/// An unset, unreadable or unlimited quota is `None` — nothing to judge capacity
/// against, which the report says plainly rather than filling in.
fn recorded_quota(quota: &str, usage_at_start: i64) -> Option<Recorded> {
    let cap = size_of(quota).filter(|cap| *cap > 0)?;
    Some(Recorded {
        cap,
        from: u64::try_from(usage_at_start).unwrap_or(0),
    })
}

/// `SABnzbd`'s size notation — a number with an optional `K`/`M`/`G`/`T`/`P` — as
/// bytes.
///
/// Each step is 1024, not 1000, which is what the client itself does: reading `100G`
/// as a hundred billion bytes would put every figure lemonfiber reports seven percent
/// away from the one the operator sees in their own client, and a figure that
/// disagrees with the client is worse than no figure. The fraction is folded in by
/// integer arithmetic rather than a float, so a size the client accepts is not
/// rounded on the way through.
fn size_of(text: &str) -> Option<u64> {
    let text = text.trim();
    let split = text
        .find(|character: char| character.is_ascii_alphabetic())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let scale: u64 = match unit.trim().to_ascii_uppercase().as_str() {
        "" => 1,
        "K" => 1 << 10,
        "M" => 1 << 20,
        "G" => 1 << 30,
        "T" => 1 << 40,
        "P" => 1 << 50,
        _ => return None,
    };
    let (whole, fraction) = number.trim().split_once('.').unwrap_or((number.trim(), ""));
    let bytes = whole.trim().parse::<u64>().ok()?.checked_mul(scale)?;
    if fraction.is_empty() {
        return Some(bytes);
    }
    let places = u32::try_from(fraction.len()).ok()?;
    let scaled = fraction.parse::<u64>().ok()?.checked_mul(scale)?;
    bytes.checked_add(scaled / 10_u64.checked_pow(places)?)
}

/// The per-day figures, as dates. A key that is not a date is dropped rather than
/// guessed at: a day nobody can place cannot be part of a window.
fn daily_totals(stats: &ServerStats) -> Vec<(Date, u64)> {
    stats
        .daily
        .iter()
        .filter_map(|(day, bytes)| Date::parse(day).map(|day| (day, *bytes)))
        .collect()
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
mod client_tests {
    use std::sync::Arc;
    use std::time::Duration;

    use lemonfiber_manifest::Date;

    use crate::ports::service::{Failure, Recorded, Transfers, UsenetAccount, UsenetAccounts};
    use crate::test_support::ScriptedHttp;

    use super::{recorded_quota, size_of, Sabnzbd};

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

    /// Two accounts as the client holds them: a block with an allowance, an expiry
    /// and a history, and an unlimited one the operator disabled and named nothing.
    const SERVERS: &str = r#"{"config":{"servers":[
        {"name":"news.example.com","displayname":"Block 500","enable":1,"quota":"500 G",
         "usage_at_start":100,"expire_date":"2026-09-01","username":"someone","password":"****"},
        {"name":"backup.example.com","displayname":"","enable":0,"quota":"","usage_at_start":0,
         "expire_date":""}
    ]}}"#;

    /// The matching statistics, keyed by the account's own name. One day's key is not
    /// a date, and the disabled account has never been used at all.
    const STATS: &str = r#"{"total":9,"servers":{"news.example.com":{
        "total":2048,"month":10,"week":5,
        "daily":{"2026-08-14":600,"2026-08-15":400,"not-a-day":999}}}}"#;

    /// The whole shape at once: what was recorded, what was measured, and the join
    /// between them — including the account the statistics have never mentioned.
    #[tokio::test]
    async fn each_account_carries_what_was_recorded_and_what_was_measured() {
        let sab = client(vec![(200, SERVERS), (200, STATS)]);
        assert_eq!(
            sab.accounts().await.unwrap_or_default(),
            vec![
                UsenetAccount {
                    name: "Block 500".to_owned(),
                    enabled: true,
                    quota: Some(Recorded {
                        cap: 500 * (1 << 30),
                        from: 100,
                    }),
                    downloaded: 2048,
                    // A day nobody can place is dropped rather than guessed at.
                    daily: vec![
                        (
                            Date {
                                year: 2026,
                                month: 8,
                                day: 14
                            },
                            600
                        ),
                        (
                            Date {
                                year: 2026,
                                month: 8,
                                day: 15
                            },
                            400
                        ),
                    ],
                    expires_on: Some(Date {
                        year: 2026,
                        month: 9,
                        day: 1
                    }),
                },
                UsenetAccount {
                    name: "backup.example.com".to_owned(),
                    enabled: false,
                    quota: None,
                    downloaded: 0,
                    daily: Vec::new(),
                    expires_on: None,
                },
            ]
        );
    }

    /// A client that writes no enabled flag at all leaves the account in use: dropping
    /// a provider silently is the worse of the two errors.
    #[tokio::test]
    async fn an_account_with_no_enabled_flag_is_read_as_one_in_use() {
        let sparse = r#"{"config":{"servers":[{"name":"news.example.com"}]}}"#;
        let sab = client(vec![(200, sparse), (200, r#"{"servers":{}}"#)]);
        assert_eq!(
            sab.accounts().await.unwrap_or_default(),
            vec![UsenetAccount {
                name: "news.example.com".to_owned(),
                enabled: true,
                quota: None,
                downloaded: 0,
                daily: Vec::new(),
                expires_on: None,
            }]
        );
    }

    #[tokio::test]
    async fn accounts_that_cannot_be_read_are_a_failure_rather_than_an_empty_list() {
        let unreadable = client(vec![(200, "not json")]);
        assert!(matches!(
            unreadable.accounts().await,
            Err(Failure::Refused { .. })
        ));

        let no_statistics = client(vec![(200, SERVERS)]);
        assert!(matches!(
            no_statistics.accounts().await,
            Err(Failure::Unavailable { .. })
        ));
    }

    /// The client steps its sizes by 1024 whatever the letter suggests, so a figure
    /// read here has to match the one the operator sees in their own client.
    #[test]
    fn a_recorded_size_reads_the_way_the_client_wrote_it() {
        for (written, bytes) in [
            ("1024", Some(1024_u64)),
            ("1K", Some(1 << 10)),
            ("10 M", Some(10 * (1 << 20))),
            ("100G", Some(100 * (1 << 30))),
            ("1.5G", Some(1024 * 1024 * 1024 + 512 * 1024 * 1024)),
            ("2T", Some(2 * (1 << 40))),
            ("1P", Some(1 << 50)),
            ("", None),
            ("-1", None),
            ("unlimited", None),
            ("100Z", None),
            ("999999999999P", None),
        ] {
            assert_eq!(size_of(written), bytes, "{written:?}");
        }
    }

    /// An allowance of nothing is no allowance: it would otherwise read as an account
    /// that is permanently, provably empty.
    #[test]
    fn an_allowance_of_zero_is_not_an_allowance() {
        assert_eq!(recorded_quota("0", 0), None);
        assert_eq!(
            recorded_quota("1G", -5),
            Some(Recorded {
                cap: 1 << 30,
                from: 0,
            }),
            "a baseline no client would write reads as none rather than wrapping"
        );
    }
}
