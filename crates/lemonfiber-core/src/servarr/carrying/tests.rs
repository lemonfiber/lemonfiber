use super::named;
use serde_json::{json, Map, Value};

fn object(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

/// Three words for the same thing across the three services, and the record is the
/// same record whichever one it used.
#[test]
fn a_record_is_named_by_whichever_word_its_service_uses() {
    assert_eq!(
        named(&object(&json!({"title": "Taskmaster"}))),
        "Taskmaster"
    );
    assert_eq!(named(&object(&json!({"name": "an indexer"}))), "an indexer");
    assert_eq!(named(&object(&json!({"artistName": "Aphex"}))), "Aphex");
}

/// A record with no name is one nothing could be matched against on the far side,
/// so it is passed over rather than carried as a blank.
#[test]
fn a_record_with_no_name_at_all_is_named_nothing() {
    assert_eq!(named(&object(&json!({"id": 4}))), "");
}

/// The title wins where a record carries more than one, which is the word the
/// libraries use.
#[test]
fn a_title_outranks_the_other_words_for_it() {
    let both = object(&json!({"name": "slug", "title": "Taskmaster"}));
    assert_eq!(named(&both), "Taskmaster");
}

use crate::ports::service::{Carried, Carrying, Record};
use crate::servarr::Servarr;
use lemonfiber_fixtures::http::{Answer, Fake};
use std::sync::Arc;

/// A service answering with the profiles it has, and taking what is put to it.
fn holding(profiles: &'static str) -> Arc<Fake> {
    Fake::by_route(vec![
        (
            crate::ports::http::Method::Get,
            "/api/v3/qualityprofile",
            Answer::reply(200, profiles),
        ),
        (
            crate::ports::http::Method::Post,
            "/api/v3/series",
            Answer::reply(201, "{}"),
        ),
    ])
}

fn following(profile: &str) -> Carried {
    Carried {
        name: "Taskmaster".to_owned(),
        profile: Some(profile.to_owned()),
        folder: Some("/data/tv".to_owned()),
        rest: r#"{"id":5,"title":"Taskmaster","qualityProfileId":1}"#.to_owned(),
    }
}

/// The number a record carried is this service's number, looked up by the name it
/// travelled under.
#[tokio::test]
async fn a_record_is_put_under_this_services_own_number_for_that_profile() {
    let http = holding(r#"[{"id":7,"name":"HD"}]"#);
    let client = Servarr::new(
        Arc::clone(&http) as Arc<dyn crate::ports::http::Http>,
        "http://x",
        "k",
        "sonarr",
        3,
    );

    let put = client.carry(Record::Series, &following("HD")).await;
    assert!(put.is_ok(), "{put:?}");

    let body = http
        .requests()
        .into_iter()
        .find(|request| request.method == crate::ports::http::Method::Post)
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(body.contains("\"qualityProfileId\":7"), "{body}");
    assert!(!body.contains("\"id\":5"), "{body}");
}

/// A profile this service does not have is a record it cannot take, rather than
/// one to give whatever profile it defaults to.
#[tokio::test]
async fn a_profile_this_service_lacks_is_refused_rather_than_guessed_at() {
    let http = holding(r#"[{"id":7,"name":"HD"}]"#);
    let client = Servarr::new(
        http as Arc<dyn crate::ports::http::Http>,
        "http://x",
        "k",
        "sonarr",
        3,
    );

    let put = client.carry(Record::Series, &following("Bespoke")).await;
    let said = put
        .err()
        .map(|failure| failure.to_string())
        .unwrap_or_default();
    assert!(said.contains("Bespoke"), "{said}");
}

/// A body that cannot be read back is one this service is never asked to take.
#[tokio::test]
async fn a_record_that_cannot_be_read_back_is_not_put_to_the_service() {
    let http = holding("[]");
    let client = Servarr::new(
        Arc::clone(&http) as Arc<dyn crate::ports::http::Http>,
        "http://x",
        "k",
        "sonarr",
        3,
    );

    let broken = Carried {
        name: "Taskmaster".to_owned(),
        profile: None,
        folder: None,
        rest: "not json at all".to_owned(),
    };
    assert!(client.carry(Record::Series, &broken).await.is_err());
    assert!(http.requests().is_empty(), "nothing was put");
}

/// A record naming no profile crosses as it came, and asks for nothing on the way.
///
/// Not every record has a profile to carry — an indexer and a download client have
/// none — so naming none is not a record this service cannot take. Looking the
/// profiles up anyway would be a call made for a number nobody wanted, on a
/// service that can refuse it.
#[tokio::test]
async fn a_record_naming_no_profile_crosses_without_asking_for_one() {
    let http = holding("[]");
    let client = Servarr::new(
        Arc::clone(&http) as Arc<dyn crate::ports::http::Http>,
        "http://x",
        "k",
        "sonarr",
        3,
    );

    let bare = Carried {
        name: "Taskmaster".to_owned(),
        profile: None,
        folder: None,
        rest: r#"{"id":5,"title":"Taskmaster"}"#.to_owned(),
    };
    let put = client.carry(Record::Series, &bare).await;
    assert!(put.is_ok(), "{put:?}");

    let asked = http.requests();
    assert!(
        asked
            .iter()
            .all(|request| !request.url.contains("qualityprofile")),
        "a record naming no profile still asked which profiles there were"
    );
    let body = asked
        .into_iter()
        .find(|request| request.method == crate::ports::http::Method::Post)
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(!body.contains("qualityProfileId"), "{body}");
    assert!(!body.contains("rootFolderPath"), "{body}");
}
