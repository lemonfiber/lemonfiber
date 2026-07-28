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

use crate::endpoint::{describe, Endpoint};
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::Failure;

/// The service name a failure is reported against.
const SERVICE: &str = "qbittorrent";

/// The content type qBittorrent's web UI API expects its form bodies as.
const FORM: &str = "application/x-www-form-urlencoded";

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
}

impl Qbittorrent {
    /// A client for the qBittorrent reached at `base`.
    #[must_use]
    pub fn new(http: Arc<dyn Http>, base: impl Into<String>) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, SERVICE),
        }
    }

    /// A form-bodied POST to a path under the web UI API.
    fn post(&self, path: &str, fields: &[(&str, &str)]) -> Request {
        Request {
            method: Method::Post,
            url: self.endpoint.url(&format!("/api/v2{path}")),
            headers: vec![("Content-Type".to_owned(), FORM.to_owned())],
            body: Some(encode(fields)),
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
        if !response.is_success() {
            return Err(self.endpoint.refusal(&response));
        }

        self.login(new).await
    }
}

/// Render form fields as an `application/x-www-form-urlencoded` body.
///
/// Infallible by construction, so there is no encoding error to fold into the
/// result: every field is a string, and every string has an encoding.
fn encode(fields: &[(&str, &str)]) -> String {
    let mut form = form_urlencoded::Serializer::new(String::new());
    for (name, value) in fields {
        form.append_pair(name, value);
    }
    form.finish()
}
