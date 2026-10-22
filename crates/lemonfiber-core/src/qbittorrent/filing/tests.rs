use std::sync::Arc;

use lemonfiber_fixtures::http::Fake;

use super::Qbittorrent;
use crate::ports::service::{Failure, Filed, Filing};
use crate::test_support::a_password;

/// A client answering each call from `replies` in order, and the transport it asks.
fn client(replies: Vec<(u16, &'static str)>) -> (Qbittorrent, Arc<Fake>) {
    let fake = Fake::scripted(replies);
    let client = Qbittorrent::authenticated(fake.clone(), "http://127.0.0.1:8080", a_password());
    (client, fake)
}

/// Categories as the client lists them: one filed under the shared folder, one below
/// the default save path, and one naming no folder.
const CATEGORIES: &str = r#"{
    "tv":{"name":"tv","savePath":"/data/downloads/complete/tv"},
    "movies":{"name":"movies","savePath":"films"},
    "music":{"name":"music","savePath":""}}"#;

/// Torrents managed automatically, saved by default under the client's own folder.
const MANAGED: &str =
    r#"{"save_path":"/data/downloads/torrents/complete/","auto_tmm_enabled":true}"#;

/// The same, with torrents managed by hand.
const BY_HAND: &str =
    r#"{"save_path":"/data/downloads/torrents/complete","auto_tmm_enabled":false}"#;

fn filed(category: &str, folder: &str) -> Filed {
    Filed {
        category: category.to_owned(),
        folder: folder.to_owned(),
    }
}

#[tokio::test]
async fn each_category_is_read_as_the_folder_its_torrents_complete_into() {
    let (torrent, _) = client(vec![(200, CATEGORIES), (200, MANAGED)]);

    assert_eq!(
        torrent.filed().await.ok(),
        Some(vec![
            filed("movies", "/data/downloads/torrents/complete/films"),
            filed("music", "/data/downloads/torrents/complete/music"),
            filed("tv", "/data/downloads/complete/tv"),
        ])
    );
}

#[tokio::test]
async fn torrents_managed_by_hand_are_read_as_completing_into_the_default_folder() {
    let (torrent, _) = client(vec![(200, CATEGORIES), (200, BY_HAND)]);

    let held = torrent.filed().await.unwrap_or_default();

    assert_eq!(held.len(), 3);
    assert!(
        held.iter()
            .all(|one| one.folder == "/data/downloads/torrents/complete"),
        "{held:?}"
    );
}

#[tokio::test]
async fn a_held_category_is_edited_a_new_one_made_and_automatic_management_switched_on() {
    let (torrent, http) = client(vec![
        (200, r#"{"tv":{"name":"tv","savePath":""}}"#),
        (200, ""),
        (200, ""),
        (200, BY_HAND),
        (200, ""),
    ]);

    let written = torrent
        .file_under(&[
            filed("tv", "/data/downloads/complete/tv"),
            filed("music", "/data/downloads/complete/music"),
        ])
        .await;

    assert_eq!(written.ok(), Some(()));
    let sent: Vec<(String, Option<String>)> = http
        .requests()
        .into_iter()
        .map(|one| (one.url, one.body))
        .collect();
    assert_eq!(
        sent.get(1),
        Some(&(
            "http://127.0.0.1:8080/api/v2/torrents/editCategory".to_owned(),
            Some("category=tv&savePath=%2Fdata%2Fdownloads%2Fcomplete%2Ftv".to_owned())
        ))
    );
    assert_eq!(
        sent.get(2),
        Some(&(
            "http://127.0.0.1:8080/api/v2/torrents/createCategory".to_owned(),
            Some("category=music&savePath=%2Fdata%2Fdownloads%2Fcomplete%2Fmusic".to_owned())
        ))
    );
    assert!(sent
        .last()
        .is_some_and(|(url, body)| url.ends_with("/app/setPreferences")
            && body
                .as_deref()
                .is_some_and(|body| body.contains("auto_tmm_enabled"))));
}

#[tokio::test]
async fn automatic_management_already_on_is_left_alone() {
    let (torrent, http) = client(vec![(200, "{}"), (200, ""), (200, MANAGED)]);

    let written = torrent
        .file_under(&[filed("tv", "/data/downloads/complete/tv")])
        .await;

    assert_eq!(written.ok(), Some(()));
    assert!(!http
        .requests()
        .iter()
        .any(|one| one.url.ends_with("/app/setPreferences")));
}

#[tokio::test]
async fn a_category_the_client_refuses_stops_the_filing_and_says_which() {
    let (torrent, http) = client(vec![(200, "{}"), (409, "")]);

    let written = torrent
        .file_under(&[
            filed("tv", "/data/downloads/complete/tv"),
            filed("music", "/data/downloads/complete/music"),
        ])
        .await;

    assert!(matches!(
        written,
        Err(Failure::Refused { detail, .. }) if detail.contains("tv")
    ));
    assert_eq!(http.requests().len(), 2);
}
