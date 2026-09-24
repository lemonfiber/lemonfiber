use crate::test_support::a_password;
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
        a_password(),
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
        Qbittorrent::authenticated(transport.clone(), "http://127.0.0.1:8080", a_password())
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
        Qbittorrent::authenticated(transport.clone(), "http://127.0.0.1:8080", a_password())
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
