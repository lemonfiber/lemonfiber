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

    /// Sign in with the recorded password, or say there is none to sign in with.
    async fn signed_in(&self) -> Result<(), Failure> {
        let password = self
            .password
            .as_deref()
            .ok_or_else(|| self.endpoint.unauthorised())?;
        self.login(password).await
    }

    /// A GET under the web UI API, sent and returned whole.
    async fn get(&self, path: &str) -> Result<crate::ports::http::Response, Failure> {
        let request = Request {
            method: Method::Get,
            url: self.endpoint.url(&format!("/api/v2{path}")),
            headers: Vec::new(),
            body: None,
        };
        self.endpoint.send(&request).await
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
    pub(crate) async fn stop_seeding(&self, name: &str) -> Result<(), Failure> {
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
        transfers(self).await
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
        seeding(self).await
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

mod fetching;
mod throttling;

async fn transfers(qbittorrent: &Qbittorrent) -> Result<Vec<Download>, Failure> {
    let Some(password) = qbittorrent.password.as_deref() else {
        return Err(qbittorrent.endpoint.unauthorised());
    };
    qbittorrent.login(password).await?;

    let request = Request {
        method: Method::Get,
        url: qbittorrent
            .endpoint
            .url("/api/v2/torrents/info?filter=downloading"),
        headers: Vec::new(),
        body: None,
    };
    let response = qbittorrent.endpoint.send(&request).await?;
    let torrents: Vec<TorrentInfo> = qbittorrent
        .endpoint
        .decode(&response, "the torrent list could not be read")?;
    Ok(torrents.into_iter().map(download_of).collect())
}

async fn seeding(qbittorrent: &Qbittorrent) -> Result<Vec<Seeded>, Failure> {
    let Some(password) = qbittorrent.password.as_deref() else {
        return Err(qbittorrent.endpoint.unauthorised());
    };
    qbittorrent.login(password).await?;
    let torrents = qbittorrent.completed().await?;
    Ok(torrents.into_iter().map(seeded_of).collect())
}

#[cfg(test)]
mod tests;
