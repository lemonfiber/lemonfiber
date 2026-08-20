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
}

mod accounts;
mod queue;

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

    use crate::ports::service::{
        Failure, Recorded, Standing, Transfers, UsenetAccount, UsenetAccounts,
    };
    use crate::test_support::ScriptedHttp;

    use super::accounts::{recorded_quota, size_of};
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

    /// The client's live view of the same two accounts: it is holding two connections
    /// to the block of the eight it is set to open, and the provider has refused the
    /// rest. The disabled one it has not built a connection to at all.
    const STATUS: &str = r#"{"status":{"servers":[
        {"servername":"Block 500","serveractive":true,"serveractiveconn":2,"servertotalconn":8,
         "servererror":"Too many connections to server news.example.com [502 Too many connections]"}
    ]}}"#;

    /// The whole shape at once: what was recorded, what was measured, how it is being
    /// served, and the join between them — including the account the statistics have
    /// never mentioned and the one the live view does not hold.
    #[tokio::test]
    async fn each_account_carries_what_was_recorded_and_what_was_measured() {
        let sab = client(vec![(200, SERVERS), (200, STATS), (200, STATUS)]);
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
                    standing: Some(Standing {
                        ready: 2,
                        configured: 8,
                        serving: true,
                        trouble: Some(
                            "Too many connections to server news.example.com [502 Too many connections]"
                                .to_owned()
                        ),
                    }),
                },
                UsenetAccount {
                    name: "backup.example.com".to_owned(),
                    enabled: false,
                    quota: None,
                    downloaded: 0,
                    daily: Vec::new(),
                    expires_on: None,
                    standing: None,
                },
            ]
        );
    }

    /// A live view that says only which account it is leaves the account in rotation
    /// with nothing recorded against it: the reading that follows from a missing flag
    /// must be the one that invents no fault.
    #[tokio::test]
    async fn a_live_view_with_nothing_filled_in_reads_as_an_account_in_rotation() {
        let bare = r#"{"status":{"servers":[{"servername":"news.example.com"}]}}"#;
        let sab = client(vec![
            (
                200,
                r#"{"config":{"servers":[{"name":"news.example.com"}]}}"#,
            ),
            (200, r#"{"servers":{}}"#),
            (200, bare),
        ]);
        assert_eq!(
            sab.accounts()
                .await
                .unwrap_or_default()
                .first()
                .and_then(|account| account.standing.clone()),
            Some(Standing {
                ready: 0,
                configured: 0,
                serving: true,
                trouble: None,
            })
        );
    }

    /// Counts that cannot be what they claim to be are read as none held: the figures
    /// are evidence of an account working, so the cautious reading is the smaller one.
    #[tokio::test]
    async fn connection_counts_that_make_no_sense_are_read_as_none_held() {
        let impossible = r#"{"status":{"servers":[{"servername":"news.example.com",
            "serveractiveconn":-4,"servertotalconn":-8,"servererror":"   "}]}}"#;
        let sab = client(vec![
            (
                200,
                r#"{"config":{"servers":[{"name":"news.example.com"}]}}"#,
            ),
            (200, r#"{"servers":{}}"#),
            (200, impossible),
        ]);
        assert_eq!(
            sab.accounts()
                .await
                .unwrap_or_default()
                .first()
                .and_then(|account| account.standing.clone()),
            Some(Standing {
                ready: 0,
                configured: 0,
                serving: true,
                trouble: None,
            })
        );
    }

    /// A client that writes no enabled flag at all leaves the account in use: dropping
    /// a provider silently is the worse of the two errors.
    #[tokio::test]
    async fn an_account_with_no_enabled_flag_is_read_as_one_in_use() {
        let sparse = r#"{"config":{"servers":[{"name":"news.example.com"}]}}"#;
        let sab = client(vec![
            (200, sparse),
            (200, r#"{"servers":{}}"#),
            (200, r#"{"status":{}}"#),
        ]);
        assert_eq!(
            sab.accounts().await.unwrap_or_default(),
            vec![UsenetAccount {
                name: "news.example.com".to_owned(),
                enabled: true,
                quota: None,
                downloaded: 0,
                daily: Vec::new(),
                expires_on: None,
                standing: None,
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

        // A client that will not say how the accounts are being served is a client
        // half-read, and half a picture reported as a whole one is the failure this
        // check exists to prevent.
        let no_live_view = client(vec![(200, SERVERS), (200, STATS)]);
        assert!(matches!(
            no_live_view.accounts().await,
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
