use lemonfiber_fixtures::http::Fake;

use super::{Filed, Filing, Sabnzbd};
use crate::ports::service::Failure;

/// A client whose transport answers each call from `replies` in order.
fn client(replies: Vec<(u16, &'static str)>) -> (Sabnzbd, std::sync::Arc<Fake>) {
    let http = Fake::scripted(replies);
    (
        Sabnzbd::new(http.clone(), "http://127.0.0.1:8080", "the-key"),
        http,
    )
}

/// Categories as the client holds them: its own for downloads given none, one filed
/// under the shared folder, one below its completed folder, and one naming no folder.
const CATEGORIES: &str = r#"{"config":{"categories":[
    {"name":"*","dir":""},
    {"name":"tv","dir":"/data/downloads/complete/tv"},
    {"name":"movies","dir":"films"},
    {"name":"music","dir":""}]}}"#;

/// The client's completed folder, as it resolves it.
const COMPLETED: &str = r#"{"status":{"completedir":"/data/downloads/usenet/complete/"}}"#;

fn filed(category: &str, folder: &str) -> Filed {
    Filed {
        category: category.to_owned(),
        folder: folder.to_owned(),
    }
}

#[tokio::test]
async fn each_category_is_read_as_the_folder_its_downloads_complete_into() {
    let (sab, _) = client(vec![(200, CATEGORIES), (200, COMPLETED)]);

    assert_eq!(
        sab.filed().await.ok(),
        Some(vec![
            filed("tv", "/data/downloads/complete/tv"),
            filed("movies", "/data/downloads/usenet/complete/films"),
            filed("music", "/data/downloads/usenet/complete/music"),
        ])
    );
}

#[tokio::test]
async fn the_completed_folder_is_the_one_the_client_resolves_rather_than_its_setting() {
    let (sab, http) = client(vec![(200, CATEGORIES), (200, COMPLETED)]);

    let _ = sab.filed().await;

    let asked: Vec<String> = http.requests().into_iter().map(|one| one.url).collect();
    assert!(
        asked
            .get(1)
            .is_some_and(|url| url.contains("mode=fullstatus&skip_dashboard=1")),
        "{asked:?}"
    );
}

#[tokio::test]
async fn categories_that_will_not_read_are_said_rather_than_read_as_none() {
    let (sab, _) = client(vec![(500, "")]);

    assert!(sab.filed().await.is_err());
}

#[tokio::test]
async fn each_category_is_written_with_its_folder_and_nothing_else() {
    let (sab, http) = client(vec![(200, "{}"), (200, r#"{"status":true}"#)]);

    let written = sab
        .file_under(&[
            filed("tv", "/data/downloads/complete/tv"),
            filed("music", "/data/downloads/complete/music"),
        ])
        .await;

    assert_eq!(written.ok(), Some(()));
    let asked: Vec<String> = http.requests().into_iter().map(|one| one.url).collect();
    assert_eq!(
        asked,
        [
            "http://127.0.0.1:8080/api?mode=set_config&section=categories&keyword=tv&dir=%2Fdata%2Fdownloads%2Fcomplete%2Ftv&output=json&apikey=the-key",
            "http://127.0.0.1:8080/api?mode=set_config&section=categories&keyword=music&dir=%2Fdata%2Fdownloads%2Fcomplete%2Fmusic&output=json&apikey=the-key",
        ]
    );
}

#[tokio::test]
async fn a_category_the_client_refuses_stops_the_filing_and_says_which() {
    let (sab, http) = client(vec![(200, r#"{"status":false,"error":"no"}"#)]);

    let written = sab
        .file_under(&[
            filed("tv", "/data/downloads/complete/tv"),
            filed("music", "/data/downloads/complete/music"),
        ])
        .await;

    assert!(matches!(
        written,
        Err(Failure::Refused { detail, .. }) if detail.contains("tv")
    ));
    assert_eq!(http.requests().len(), 1);
}
