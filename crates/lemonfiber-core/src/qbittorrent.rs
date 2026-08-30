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
use crate::ports::service::{Download, Failure, Transfers};

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
            &[("username", "admin"), ("password", password)],
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::ports::service::{Failure, Transfers};
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

    #[tokio::test]
    async fn a_client_holding_no_password_cannot_authenticate_a_read() {
        let qbit = Qbittorrent::new(Fake::scripted(Vec::new()), "http://127.0.0.1:8080");
        assert!(matches!(
            qbit.transfers().await,
            Err(Failure::Unauthorised { .. })
        ));
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

    #[tokio::test]
    async fn a_refused_write_is_reported_rather_than_read_back() {
        let client = client(vec![LOGGED_IN, (403, "Forbidden")]);
        assert!(client.set_listen_port(51413).await.is_err());
    }
}
