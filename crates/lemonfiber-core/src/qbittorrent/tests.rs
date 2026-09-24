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
