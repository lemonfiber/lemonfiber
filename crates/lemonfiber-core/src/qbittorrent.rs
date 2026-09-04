//! Speaking qBittorrent's web UI API.
//!
//! qBittorrent is the one service lemonfiber gives a credential to rather than
//! reading one from. It mints a throwaway password on each start, announces it in
//! its log, and asks for it to be replaced. So this reads that announced password
//! from the log, authenticates with it, sets a durable one lemonfiber generated,
//! and confirms the change by authenticating again with the new one.
//!
//! Authentication is a session: the login call sets a cookie the transport
//! carries onto the calls that follow, so the code here never handles the cookie
//! itself — that is the adapter's job. Success and failure are read from the
//! service's own words as much as its status, because a qBittorrent login answers
//! `200` whether the password was right (`Ok.`) or wrong (`Fails.`).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use crate::dashboard::percent;
use crate::endpoint::{describe, form_content_type, form_encoded, Endpoint};
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::{Download, Failure, Seeded, Seeding, Transfers};

/// The service name a failure is reported against.
const SERVICE: &str = "qbittorrent";

/// The phrase qBittorrent logs its temporary password after.
const TEMP_MARKER: &str = "A temporary password is provided for this session:";

/// qBittorrent's temporary web UI password, read from its startup log, if it
/// announced one.
///
/// The most recent announcement wins: the log is scanned from the end, so a
/// restart's fresh password is taken rather than a stale earlier one. An
/// announcement with nothing after it is treated as no password — a truncated or
/// half-written line, not something to authenticate with.
#[must_use]
pub fn temporary_password(log: &str) -> Option<String> {
    log.lines()
        .rev()
        .find_map(|line| line.split_once(TEMP_MARKER))
        .map(|(_, password)| password.trim().to_owned())
        .filter(|password| !password.is_empty())
}

/// A client for one qBittorrent web UI.
pub struct Qbittorrent {
    endpoint: Endpoint,
    /// The durable password to authenticate a read with, where one is held. The
    /// password-replacement flow is given the current and new passwords per call
    /// and needs none stored; the dashboard's transfers read holds the recorded
    /// one, so a client built for one purpose cannot silently be used for the
    /// other without a password to prove itself.
    password: Option<String>,
}

impl Qbittorrent {
    /// A client for the qBittorrent reached at `base`, holding no password — for
    /// the first-run exchange that is handed each password explicitly.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, SERVICE),
            password: None,
        }
    }

    /// A client that can authenticate a read itself, holding the durable password
    /// lemonfiber recorded — how the dashboard reads its transfers.
    #[must_use]
    pub fn authenticated(
        http: Arc<dyn Http>,
        base: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, SERVICE),
            password: Some(password.into()),
        }
    }

    /// A form-bodied POST to a path under the web UI API.
    fn post(&self, path: &str, fields: &[(&str, &str)]) -> Request {
        Request {
            method: Method::Post,
            url: self.endpoint.url(&format!("/api/v2{path}")),
            headers: vec![form_content_type()],
            body: Some(form_encoded(fields)),
        }
    }

    /// Authenticate, so the session cookie the transport carries lets the calls
    /// that follow through.
    ///
    /// A wrong password is `Unauthorised`; qBittorrent says so with `Fails.` at
    /// `200`, or with `403` once it has seen too many attempts.
    async fn login(&self, password: &str) -> Result<(), Failure> {
        let request = self.post(
            "/auth/login",
            &[
                ("username", crate::config::QBITTORRENT_USER),
                ("password", password),
            ],
        );
        let response = self.endpoint.send(&request).await?;
        if response.is_success() && response.body.trim() == "Ok." {
            Ok(())
        } else if response.status == 403 || response.body.contains("Fails") {
            Err(self.endpoint.unauthorised())
        } else {
            Err(self.endpoint.refused(&describe(&response)))
        }
    }

    /// Whether qBittorrent takes this password.
    ///
    /// What tells a client already set up from one still holding its start-up
    /// credential, so a second run reports what is rather than trying to set it
    /// again.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where qBittorrent cannot be reached, or refuses it.
    pub async fn accepts(&self, password: &str) -> Result<(), Failure> {
        self.login(password).await
    }

    /// Replace the web UI password: authenticate with the current one, set the
    /// new one, and confirm it by authenticating again with the new one.
    ///
    /// The confirming login is the read-back — a set that qBittorrent accepted but
    /// did not apply is caught here rather than being called done.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where qBittorrent cannot be reached, rejects the
    /// current password, or refuses the change.
    pub async fn replace_password(&self, current: &str, new: &str) -> Result<(), Failure> {
        self.login(current).await?;

        let preferences = serde_json::json!({ "web_ui_password": new }).to_string();
        let request = self.post("/app/setPreferences", &[("json", &preferences)]);
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)?;

        self.login(new).await
    }

    /// Which port the client is listening on for incoming peers.
    ///
    /// The number that has to match what the VPN granted: peers reach a client on
    /// the port the provider forwards, and a client listening elsewhere is
    /// connectable by nobody while looking entirely healthy from inside.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where qBittorrent cannot be reached, rejects the
    /// password, or answers with something unreadable.
    pub async fn listen_port(&self) -> Result<u16, Failure> {
        let password = self
            .password
            .as_deref()
            .ok_or_else(|| self.endpoint.unauthorised())?;
        self.login(password).await?;

        let request = Request {
            method: Method::Get,
            url: self.endpoint.url("/api/v2/app/preferences"),
            headers: Vec::new(),
            body: None,
        };
        let response = self.endpoint.send(&request).await?;
        let preferences: Preferences = self
            .endpoint
            .decode(&response, "the preferences could not be read")?;
        Ok(preferences.listen_port)
    }

    /// Listen on `port` instead, and confirm the client took it.
    ///
    /// Read back rather than trusted, for the reason the password change is: a
    /// client that accepted the write and did not apply it would otherwise be
    /// recorded as configured while remaining unreachable — which is the failure
    /// this whole path exists to notice.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where qBittorrent cannot be reached, rejects the
    /// password, refuses the change, or reports a different port afterwards.
    pub async fn set_listen_port(&self, port: u16) -> Result<(), Failure> {
        let password = self
            .password
            .as_deref()
            .ok_or_else(|| self.endpoint.unauthorised())?;
        self.login(password).await?;

        let preferences = serde_json::json!({ "listen_port": port }).to_string();
        let request = self.post("/app/setPreferences", &[("json", &preferences)]);
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)?;

        match self.listen_port().await? {
            listening if listening == port => Ok(()),
            listening => Err(self.endpoint.refused(&format!(
                "the port was set to {port} and the client is listening on {listening}"
            ))),
        }
    }

    /// Every completed torrent the client is holding, as it reports them.
    ///
    /// One listing behind both the read of what is being seeded and the removal of
    /// one of them, so the two cannot come to disagree about what the client holds.
    /// It authenticates nothing: each caller logs in first, because a client holding
    /// no password is a refusal about the caller rather than about the listing.
    async fn completed(&self) -> Result<Vec<CompletedInfo>, Failure> {
        let request = Request {
            method: Method::Get,
            url: self.endpoint.url("/api/v2/torrents/info?filter=completed"),
            headers: Vec::new(),
            body: None,
        };
        let response = self.endpoint.send(&request).await?;
        self.endpoint
            .decode(&response, "the completed torrent list could not be read")
    }

    /// Stop seeding one completed download: take it out of the client, and the copy
    /// in the downloads tree with it.
    ///
    /// Asked for by the name both sides call it, because a name is what every other
    /// reading here matches on and a hash is a thing nobody reads. The hash it is
    /// addressed by is looked up in the same call, from a listing taken now, so what
    /// goes is what the client is holding at the moment it is asked rather than what
    /// it was holding when somebody read an offer.
    ///
    /// Two completed torrents of one name are refused rather than chosen between.
    /// Which was meant is not a question anything here can answer, and answering it
    /// wrongly takes the other.
    ///
    /// Confirmed by looking again, for the reason the port change is: a client that
    /// answered the removal and went on holding the torrent would otherwise have a
    /// ratio reported as spent while it is still being earned, which is the one
    /// figure this whole errand turns on.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where qBittorrent cannot be reached, rejects the password,
    /// is holding nothing of that name or more than one of it, refuses the removal,
    /// or is still holding it afterwards.
    pub async fn stop_seeding(&self, name: &str) -> Result<(), Failure> {
        let password = self
            .password
            .as_deref()
            .ok_or_else(|| self.endpoint.unauthorised())?;
        self.login(password).await?;

        let holding = self.completed().await?;
        let named: Vec<&CompletedInfo> = holding
            .iter()
            .filter(|torrent| torrent.name == name)
            .collect();
        let [one] = named.as_slice() else {
            return Err(self.endpoint.refused(&format!(
                "the client is holding {} completed downloads called {name}",
                named.len()
            )));
        };

        let request = self.post(
            "/torrents/delete",
            &[("hashes", one.hash.as_str()), ("deleteFiles", "true")],
        );
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)?;

        let after = self.completed().await?;
        if after.iter().any(|torrent| torrent.name == name) {
            return Err(self
                .endpoint
                .refused(&format!("the client is still holding {name}")));
        }
        Ok(())
    }
}

/// The preferences fields the forwarded port is read from. The many others
/// qBittorrent sends are ignored.
#[derive(Deserialize)]
struct Preferences {
    #[serde(default)]
    listen_port: u16,
}

/// qBittorrent's sentinel `eta` for "no estimate to give" — 100 days, in seconds.
/// A stalled torrent reports this rather than a real countdown, so it becomes no
/// ETA rather than one 100 days out.
const NO_ETA: u64 = 8_640_000;

/// One torrent as qBittorrent's `torrents/info` reports it.
///
/// Progress is read from the byte counts, not the `progress` float, so the
/// percentage is integer arithmetic that shares the dashboard's own `percent`.
/// The many other fields qBittorrent sends are ignored.
#[derive(Deserialize)]
struct TorrentInfo {
    name: String,
    completed: u64,
    size: u64,
    dlspeed: u64,
    eta: u64,
}

#[async_trait]
impl Transfers for Qbittorrent {
    async fn transfers(&self) -> Result<Vec<Download>, Failure> {
        let Some(password) = self.password.as_deref() else {
            return Err(self.endpoint.unauthorised());
        };
        self.login(password).await?;

        let request = Request {
            method: Method::Get,
            url: self
                .endpoint
                .url("/api/v2/torrents/info?filter=downloading"),
            headers: Vec::new(),
            body: None,
        };
        let response = self.endpoint.send(&request).await?;
        let torrents: Vec<TorrentInfo> = self
            .endpoint
            .decode(&response, "the torrent list could not be read")?;
        Ok(torrents.into_iter().map(download_of).collect())
    }
}

/// One completed torrent as `torrents/info` reports it.
///
/// A separate shape from the one an active download is read as, because the two
/// reads want different fields: what is arriving is read for its progress and its
/// speed, and what has arrived is read for what it occupies and what it has given
/// back. The many other fields qBittorrent sends are ignored by both.
/// The ratio is worked out from the two byte counts rather than read from the
/// `ratio` qBittorrent also sends, because that one is a floating-point number: it
/// arrives with the noise a decimal fraction picks up in binary, it cannot be
/// compared for equality between two runs, and qBittorrent writes `-1` in it for a
/// torrent it considers to have an infinite ratio. The counts it divides are whole
/// numbers and are the same answer without any of that.
#[derive(Deserialize)]
struct CompletedInfo {
    hash: String,
    name: String,
    size: u64,
    uploaded: u64,
    downloaded: u64,
}

#[async_trait]
impl Seeding for Qbittorrent {
    async fn seeding(&self) -> Result<Vec<Seeded>, Failure> {
        let Some(password) = self.password.as_deref() else {
            return Err(self.endpoint.unauthorised());
        };
        self.login(password).await?;
        let torrents = self.completed().await?;
        Ok(torrents.into_iter().map(seeded_of).collect())
    }
}

/// One completed torrent as the reckoning's [`Seeded`].
fn seeded_of(torrent: CompletedInfo) -> Seeded {
    Seeded {
        name: torrent.name,
        bytes: torrent.size,
        ratio: hundredths(torrent.uploaded, torrent.downloaded),
    }
}

/// What was given back against what was taken, in whole hundredths.
///
/// A torrent that downloaded nothing — added from files already on disk — has
/// given back everything against nothing, which is the case qBittorrent itself
/// writes as an infinite ratio. It becomes the largest figure this can carry,
/// which reads as what it means rather than as a division nobody can do.
fn hundredths(uploaded: u64, downloaded: u64) -> u32 {
    if downloaded == 0 {
        return u32::MAX;
    }
    let scaled = uploaded.saturating_mul(100) / downloaded;
    u32::try_from(scaled).unwrap_or(u32::MAX)
}

/// One torrent as the dashboard's [`Download`]: progress from its byte counts, its
/// download speed as reported, and the ETA only where qBittorrent gave a real one.
fn download_of(torrent: TorrentInfo) -> Download {
    Download {
        name: torrent.name,
        progress: percent(torrent.completed, torrent.size),
        speed: Some(torrent.dlspeed),
        eta: (torrent.eta < NO_ETA).then(|| Duration::from_secs(torrent.eta)),
        // What is still to land on disk — the same two byte counts progress reads,
        // subtracted rather than divided, saturating so a size a hair behind the
        // completed count never wraps.
        remaining: Some(torrent.size.saturating_sub(torrent.completed)),
    }
}

mod throttling;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::ports::service::{Failure, Seeding, Transfers};
    use crate::test_support::a_password;
    use lemonfiber_fixtures::http::Fake;

    use super::Qbittorrent;

    /// A client whose transport answers each call from `replies` in order — the
    /// first is the login, the second the torrent list.
    fn client(replies: Vec<(u16, &'static str)>) -> Qbittorrent {
        Qbittorrent::authenticated(
            Fake::scripted(replies),
            "http://127.0.0.1:8080",
            a_password(),
        )
    }

    /// Two torrents: one mid-download with a real ETA, one complete and stalled at
    /// qBittorrent's no-estimate sentinel.
    const TWO_TORRENTS: &str = r#"[
        {"name":"Show.S01E01","completed":500,"size":1000,"dlspeed":2048,"eta":600},
        {"name":"Movie.2024","completed":1000,"size":1000,"dlspeed":0,"eta":8640000}
    ]"#;

    #[tokio::test]
    async fn each_torrent_reads_its_progress_speed_and_eta() {
        let qbit = client(vec![(200, "Ok."), (200, TWO_TORRENTS)]);
        let transfers = qbit.transfers().await.unwrap_or_default();
        assert_eq!(transfers.len(), 2);
        assert!(matches!(
            transfers.first(),
            Some(t) if t.name == "Show.S01E01"
                && t.progress == 50
                && t.speed == Some(2048)
                && t.eta == Some(Duration::from_secs(600))
                && t.remaining == Some(500)
        ));
        // The sentinel ETA becomes no estimate, not a countdown 100 days out; a
        // complete torrent has nothing left to land — a definite zero, not unknown.
        assert!(matches!(
            transfers.get(1),
            Some(t) if t.progress == 100 && t.speed == Some(0) && t.eta.is_none()
                && t.remaining == Some(0)
        ));
    }

    /// Three completed torrents: one that has given back more than it took, one
    /// that has given back almost nothing, and one added from files already on
    /// disk, which downloaded nothing at all.
    const THREE_COMPLETED: &str = r#"[
        {"hash":"aa","name":"Show.S01E01","size":1000,"uploaded":1750,"downloaded":1000},
        {"hash":"bb","name":"Movie.2024","size":4000,"uploaded":7,"downloaded":4000},
        {"hash":"cc","name":"Already.Here","size":9000,"uploaded":100,"downloaded":0}
    ]"#;

    #[tokio::test]
    async fn each_completed_torrent_reads_what_it_holds_and_what_it_has_given_back() {
        let qbit = client(vec![(200, "Ok."), (200, THREE_COMPLETED)]);
        let held = qbit.seeding().await.unwrap_or_default();
        assert_eq!(held.len(), 3);
        assert!(matches!(
            held.first(),
            Some(one) if one.name == "Show.S01E01" && one.bytes == 1000 && one.ratio == 175
        ));
        // Worked out from the byte counts rather than read off the float beside
        // them: a seventh of a percent is a figure, not noise.
        assert!(matches!(held.get(1), Some(one) if one.ratio == 0));
        // Nothing downloaded is a ratio nobody can divide, and it reads as having
        // given back far more than it took rather than as an error.
        assert!(matches!(held.get(2), Some(one) if one.ratio == u32::MAX));
    }

    #[tokio::test]
    async fn a_client_holding_no_password_cannot_authenticate_a_read() {
        let qbit = Qbittorrent::new(Fake::scripted(Vec::new()), "http://127.0.0.1:8080");
        assert!(matches!(
            qbit.transfers().await,
            Err(Failure::Unauthorised { .. })
        ));
        assert!(matches!(
            qbit.seeding().await,
            Err(Failure::Unauthorised { .. })
        ));
    }

    #[tokio::test]
    async fn a_completed_read_that_is_refused_or_unanswered_says_which() {
        let refused = client(vec![(200, "Fails.")]);
        assert!(matches!(
            refused.seeding().await,
            Err(Failure::Unauthorised { .. })
        ));
        let silent = client(vec![(200, "Ok.")]);
        assert!(matches!(
            silent.seeding().await,
            Err(Failure::Unavailable { .. })
        ));
        let nonsense = client(vec![(200, "Ok."), (200, "not a torrent list")]);
        assert!(nonsense.seeding().await.is_err(), "unreadable is not empty");
    }

    #[tokio::test]
    async fn a_rejected_password_is_unauthorised() {
        let qbit = client(vec![(200, "Fails.")]);
        assert!(matches!(
            qbit.transfers().await,
            Err(Failure::Unauthorised { .. })
        ));
    }

    #[tokio::test]
    async fn a_client_that_stops_answering_after_login_is_unavailable() {
        let qbit = client(vec![(200, "Ok.")]);
        assert!(matches!(
            qbit.transfers().await,
            Err(Failure::Unavailable { .. })
        ));
    }

    #[tokio::test]
    async fn a_torrent_list_that_will_not_parse_is_refused() {
        let qbit = client(vec![(200, "Ok."), (200, "not json")]);
        assert!(matches!(
            qbit.transfers().await,
            Err(Failure::Refused { .. })
        ));
    }

    /// A successful login, as every call begins with.
    const LOGGED_IN: (u16, &str) = (200, "Ok.");

    #[tokio::test]
    async fn the_listening_port_is_read_from_the_preferences() {
        // The number that has to match what the VPN granted: peers reach a client
        // on the port the provider forwards.
        let client = client(vec![LOGGED_IN, (200, r#"{"listen_port":51413}"#)]);
        assert_eq!(client.listen_port().await.ok(), Some(51413));
    }

    #[tokio::test]
    async fn preferences_that_will_not_parse_are_refused_rather_than_guessed() {
        let client = client(vec![LOGGED_IN, (200, "not json")]);
        assert!(client.listen_port().await.is_err());
    }

    #[tokio::test]
    async fn a_client_holding_no_password_cannot_read_or_set_the_port() {
        let anonymous = Qbittorrent::new(Fake::scripted(Vec::new()), "http://127.0.0.1:8080");
        assert!(anonymous.listen_port().await.is_err());
        assert!(anonymous.set_listen_port(51413).await.is_err());
    }

    #[tokio::test]
    async fn setting_the_port_is_confirmed_by_reading_it_back() {
        // Read back rather than trusted: a client that accepted the write and did
        // not apply it would otherwise be recorded as configured while remaining
        // unreachable, which is the failure this whole path exists to notice.
        let client = client(vec![
            LOGGED_IN,
            (200, ""),
            LOGGED_IN,
            (200, r#"{"listen_port":51413}"#),
        ]);
        assert!(client.set_listen_port(51413).await.is_ok());
    }

    #[tokio::test]
    async fn a_client_that_took_the_write_and_kept_its_old_port_is_a_failure() {
        // Accepted and not applied — the case the read-back exists for.
        let client = client(vec![
            LOGGED_IN,
            (200, ""),
            LOGGED_IN,
            (200, r#"{"listen_port":6881}"#),
        ]);
        let refused = client.set_listen_port(51413).await;
        assert!(
            refused.is_err(),
            "the client is not on the port it was set to"
        );
    }

    /// One completed torrent, and the same name twice, for the removal's own cases.
    const ONE_COMPLETED: &str = r#"[{"hash":"a1","name":"Show.S01E01","size":1000,
        "uploaded":1750,"downloaded":1000}]"#;

    /// Two completed torrents of one name, which is a question nothing here can
    /// answer and a wrong answer removes the other one.
    const TWO_OF_A_NAME: &str = r#"[
        {"hash":"a1","name":"Show.S01E01","size":1000,"uploaded":1750,"downloaded":1000},
        {"hash":"b2","name":"Show.S01E01","size":1000,"uploaded":10,"downloaded":1000}
    ]"#;

    /// Nothing at all, which is what the listing says once the torrent has gone.
    const NONE_COMPLETED: (u16, &str) = (200, "[]");

    /// A write qBittorrent accepted, which it answers with an empty body.
    const ACCEPTED: (u16, &str) = (200, "");

    #[tokio::test]
    async fn a_torrent_let_go_is_addressed_by_hash_and_read_back_as_gone() {
        // Addressed by the hash a listing taken now reports, rather than by anything
        // somebody read earlier, and confirmed by looking again: a client that
        // answered the removal and went on holding it would have a ratio recorded as
        // lost while it is still being earned.
        let client = client(vec![
            LOGGED_IN,
            (200, ONE_COMPLETED),
            ACCEPTED,
            NONE_COMPLETED,
        ]);
        assert!(client.stop_seeding("Show.S01E01").await.is_ok());
    }

    #[tokio::test]
    async fn a_client_still_holding_it_afterwards_is_a_failure_rather_than_a_removal() {
        let client = client(vec![
            LOGGED_IN,
            (200, ONE_COMPLETED),
            ACCEPTED,
            (200, ONE_COMPLETED),
        ]);
        let refused = client.stop_seeding("Show.S01E01").await;
        assert!(
            refused.is_err(),
            "it is still seeding and the room is still spent"
        );
    }

    #[tokio::test]
    async fn a_name_the_client_no_longer_holds_is_refused_rather_than_guessed_at() {
        // Between an offer being read and being answered a torrent can finish and be
        // gone, and the listing this takes now is what says so.
        let client = client(vec![LOGGED_IN, NONE_COMPLETED]);
        assert!(client.stop_seeding("Show.S01E01").await.is_err());
    }

    #[tokio::test]
    async fn two_completed_torrents_of_one_name_are_refused_rather_than_chosen_between() {
        let client = client(vec![LOGGED_IN, (200, TWO_OF_A_NAME)]);
        assert!(
            client.stop_seeding("Show.S01E01").await.is_err(),
            "answering which was meant wrongly removes the other"
        );
    }

    #[tokio::test]
    async fn a_removal_the_client_refuses_is_reported_rather_than_read_back() {
        let client = client(vec![LOGGED_IN, (200, ONE_COMPLETED), (403, "Forbidden")]);
        assert!(client.stop_seeding("Show.S01E01").await.is_err());
    }

    #[tokio::test]
    async fn a_client_holding_no_password_cannot_ask_for_anything_to_be_removed() {
        let anonymous = Qbittorrent::new(Fake::scripted(Vec::new()), "http://127.0.0.1:8080");
        assert!(matches!(
            anonymous.stop_seeding("Show.S01E01").await,
            Err(Failure::Unauthorised { .. })
        ));
    }

    #[tokio::test]
    async fn a_refused_write_is_reported_rather_than_read_back() {
        let client = client(vec![LOGGED_IN, (403, "Forbidden")]);
        assert!(client.set_listen_port(51413).await.is_err());
    }
}
