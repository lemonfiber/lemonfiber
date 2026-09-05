//! Stopping qBittorrent fetching, and starting it again.
//!
//! Two writes each way rather than one, because this client has no single switch
//! and the half that looks like one is not. Stopping every torrent stops what is
//! running; it does nothing about the next release an \*arr hands over, which the
//! client starts by itself within the hour. So the preference that adds new
//! torrents already stopped is written beside it, and the two together are what
//! "nothing new is fetched" comes to here.
//!
//! Both are read back, and the read-back is the same pair: nothing running, and
//! nothing new would start. A client answering `200` to both writes and going on
//! fetching is exactly the failure a cap exists to prevent.
//!
//! The endpoint names are this client's own and were taken from the pinned image
//! rather than from a document: at this web API version the pair is `stop` and
//! `start`, and the `pause` and `resume` that a great deal of writing still names
//! answer `404`.

use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::service::{Failure, Fetching, Pulling};

use super::Qbittorrent;

/// Every torrent the client holds, addressed at once.
const EVERY: &str = "all";

/// The preference fields whether new work would start is read from. The many
/// others qBittorrent sends are ignored.
#[derive(Deserialize)]
struct Adding {
    #[serde(default)]
    add_stopped_enabled: bool,
}

/// One torrent the client reports as running.
///
/// Nothing is read out of it: how many came back is the whole answer, and naming a
/// field would tie this read to a shape it makes no use of.
#[derive(Deserialize)]
struct Running {}

impl Qbittorrent {
    /// How many torrents the client says are running.
    async fn running(&self) -> Result<usize, Failure> {
        let response = self.get("/torrents/info?filter=running").await?;
        let running: Vec<Running> = self
            .endpoint
            .decode(&response, "what the client is running could not be read")?;
        Ok(running.len())
    }

    /// Whether the client would start the next torrent handed to it.
    async fn would_start(&self) -> Result<bool, Failure> {
        let response = self.get("/app/preferences").await?;
        let adding: Adding = self
            .endpoint
            .decode(&response, "how new downloads are added could not be read")?;
        Ok(!adding.add_stopped_enabled)
    }

    /// Write the preference that decides what happens to the next torrent added.
    async fn adding_stopped(&self, stopped: bool) -> Result<(), Failure> {
        let asked = serde_json::json!({ "add_stopped_enabled": stopped }).to_string();
        let request = self.post("/app/setPreferences", &[("json", &asked)]);
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)
    }

    /// Stop or start every torrent the client holds.
    async fn every_torrent(&self, action: &str) -> Result<(), Failure> {
        let request = self.post(&format!("/torrents/{action}"), &[("hashes", EVERY)]);
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)
    }
}

#[async_trait]
impl Fetching for Qbittorrent {
    async fn pulling(&self) -> Result<Pulling, Failure> {
        self.signed_in().await?;
        Ok(Pulling::of(
            self.running().await? > 0,
            self.would_start().await?,
        ))
    }

    async fn stop(&self) -> Result<Pulling, Failure> {
        self.signed_in().await?;
        // The preference first. Stopping what is running and then leaving a window
        // in which the next grab starts would be a pause that let one more
        // download through every time it was asked for.
        self.adding_stopped(true).await?;
        self.every_torrent("stop").await?;
        self.pulling().await
    }

    async fn resume(&self) -> Result<Pulling, Failure> {
        self.signed_in().await?;
        self.adding_stopped(false).await?;
        self.every_torrent("start").await?;
        self.pulling().await
    }
}

#[cfg(test)]
mod tests {
    use lemonfiber_fixtures::http::{Answer, Fake};

    use super::{Fetching, Pulling, Qbittorrent};
    use crate::ports::http::Method;

    /// What the client answers a login it accepted.
    const IN: &str = "Ok.";

    /// One torrent, and none, as the running filter reports them.
    const ONE: &str = r#"[{"name":"Show.S01E01"}]"#;
    const NONE: &str = "[]";

    /// The preferences of a client that starts what it is given, and of one that
    /// adds it already stopped.
    const STARTS: &str = r#"{"add_stopped_enabled":false}"#;
    const WAITS: &str = r#"{"add_stopped_enabled":true}"#;

    /// A client whose transport answers each call from `replies` in order.
    fn client(replies: Vec<(u16, &'static str)>) -> Qbittorrent {
        Qbittorrent::authenticated(
            Fake::scripted(replies),
            "http://127.0.0.1:8080",
            "the-password",
        )
    }

    #[tokio::test]
    async fn a_client_with_nothing_running_that_would_start_the_next_one_is_still_fetching() {
        // The half of "stopped" that a stop-everything call does not reach. An
        // \*arr hands the client a release within the hour, and a client that
        // starts it has not stopped.
        let held = client(vec![(200, IN), (200, NONE), (200, STARTS)])
            .pulling()
            .await;
        assert_eq!(held.ok(), Some(Pulling::Fetching));
    }

    #[tokio::test]
    async fn a_client_running_something_is_fetching_however_it_treats_new_work() {
        let held = client(vec![(200, IN), (200, ONE), (200, WAITS)])
            .pulling()
            .await;
        assert_eq!(held.ok(), Some(Pulling::Fetching));
    }

    #[tokio::test]
    async fn a_client_running_nothing_that_would_start_nothing_is_stopped() {
        let held = client(vec![(200, IN), (200, NONE), (200, WAITS)])
            .pulling()
            .await;
        assert_eq!(held.ok(), Some(Pulling::Stopped));
    }

    #[tokio::test]
    async fn stopping_it_writes_the_preference_before_it_stops_what_is_running() {
        // The other order leaves a window in which the next grab starts, which is a
        // pause that lets one more download through every time it is asked for.
        let transport = Fake::scripted(vec![
            (200, IN),
            (200, ""),
            (200, ""),
            (200, IN),
            (200, NONE),
            (200, WAITS),
        ]);
        let stopped =
            Qbittorrent::authenticated(transport.clone(), "http://127.0.0.1:8080", "the-password")
                .stop()
                .await;
        assert_eq!(stopped.ok(), Some(Pulling::Stopped));

        let posted: Vec<String> = transport
            .requests()
            .iter()
            .filter(|request| request.method == Method::Post)
            .map(|request| request.url.clone())
            .collect();
        let order = posted.join(" ");
        let preference = order.find("setPreferences");
        let torrents = order.find("torrents/stop");
        assert!(
            preference.is_some() && torrents.is_some() && preference < torrents,
            "{order}"
        );
    }

    #[tokio::test]
    async fn a_client_that_took_both_writes_and_went_on_running_says_so() {
        // The failure this path exists to notice: a `200` to each write and a
        // torrent still moving is a month still being spent.
        let going = client(vec![
            (200, IN),
            (200, ""),
            (200, ""),
            (200, IN),
            (200, ONE),
            (200, WAITS),
        ])
        .stop()
        .await;
        assert_eq!(going.ok(), Some(Pulling::Fetching));
    }

    #[tokio::test]
    async fn starting_it_again_lets_new_work_in_as_well_as_what_was_held() {
        let transport = Fake::scripted(vec![
            (200, IN),
            (200, ""),
            (200, ""),
            (200, IN),
            (200, ONE),
            (200, STARTS),
        ]);
        let going =
            Qbittorrent::authenticated(transport.clone(), "http://127.0.0.1:8080", "the-password")
                .resume()
                .await;
        assert_eq!(going.ok(), Some(Pulling::Fetching));
        assert!(transport.asked_for("torrents/start"));
    }

    #[tokio::test]
    async fn a_client_holding_no_password_is_asked_for_nothing() {
        let anonymous = Qbittorrent::new(Fake::always(Answer::reply(200, IN)), "http://127.0.0.1");
        assert!(anonymous.pulling().await.is_err());
        assert!(anonymous.stop().await.is_err());
        assert!(anonymous.resume().await.is_err());
    }

    #[tokio::test]
    async fn a_client_that_refuses_a_write_or_a_read_is_reported_rather_than_guessed_at() {
        assert!(client(vec![(200, IN), (403, "")]).stop().await.is_err());
        assert!(client(vec![(200, IN), (200, ""), (403, "")])
            .stop()
            .await
            .is_err());
        assert!(client(vec![(200, IN), (200, "not json")])
            .pulling()
            .await
            .is_err());
        assert!(client(vec![(200, IN), (200, NONE), (200, "not json")])
            .pulling()
            .await
            .is_err());
        assert!(client(vec![(200, IN), (403, "")]).resume().await.is_err());
    }
}
