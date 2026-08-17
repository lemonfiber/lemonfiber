//! The Prowlarr application-sync client, driven through the HTTP port against a
//! fake transport.
//!
//! The client turns a request into an `/api/v1/applications` call and reads what
//! Prowlarr answered; the fake is that Prowlarr, replying with exactly the status
//! and body a test wants — so every branch of the registration and read-back
//! paths is exercised with nothing running. It speaks an async trait built on
//! another, so it is driven from here rather than from an in-crate test, where it
//! would be compiled twice and its coverage counted from the wrong copy.

mod common;

use std::sync::Arc;

use common::{Answer, Fake};
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::ports::service::{
    AppSync, Application, ApplicationKind, Failure, IndexerUse, Indexers, RegisteredApplication,
};
use lemonfiber_core::prowlarr::Prowlarr;
use lemonfiber_manifest::Date;

/// The day the indexer counts are asked for, built rather than parsed: parsing has
/// its own tests, and a fixture that can fail is a second thing to reason about.
const TODAY: Date = Date {
    year: 2026,
    month: 8,
    day: 16,
};

/// A Prowlarr client over the given fake.
fn prowlarr(fake: &Arc<Fake>) -> Prowlarr {
    let http: Arc<dyn Http> = fake.clone();
    Prowlarr::new(http, "http://127.0.0.1:9696", "prowlarr-key", "prowlarr")
}

/// A wanted application of the given kind, reaching the given \*arr.
fn application(name: &str, kind: ApplicationKind, base_url: &str) -> Application {
    Application {
        name: name.to_owned(),
        kind,
        prowlarr_url: "http://prowlarr:9696".to_owned(),
        base_url: base_url.to_owned(),
        api_key: "arr-key".to_owned(),
    }
}

#[tokio::test]
async fn an_application_is_posted_to_its_v1_endpoint_with_the_key() {
    let fake = Fake::always(Answer::reply(201, ""));
    let sonarr = application("Sonarr", ApplicationKind::Sonarr, "http://sonarr:8989");
    assert!(prowlarr(&fake).register_application(&sonarr).await.is_ok());

    let sent = fake.request();
    // Prowlarr's API is a major behind the media *arrs': v1, not v3.
    assert!(sent
        .as_ref()
        .is_some_and(|request| request.url.ends_with("/api/v1/applications")));
    assert!(sent.as_ref().is_some_and(|request| request
        .headers
        .iter()
        .any(|(name, value)| name == "X-Api-Key" && value == "prowlarr-key")));
    // The JSON body is announced as such, so Prowlarr binds it rather than
    // dropping a body it was not told the type of.
    assert!(sent.is_some_and(|request| request
        .headers
        .iter()
        .any(|(name, value)| name == "Content-Type" && value == "application/json")));
}

#[tokio::test]
async fn a_sonarr_application_carries_its_schema_and_television_categories() {
    let fake = Fake::always(Answer::reply(201, ""));
    let sonarr = application("Sonarr", ApplicationKind::Sonarr, "http://sonarr:8989");
    assert!(prowlarr(&fake).register_application(&sonarr).await.is_ok());

    let body = fake
        .request()
        .and_then(|request| request.body)
        .unwrap_or_default();
    for expected in [
        r#""syncLevel":"fullSync""#,
        r#""implementation":"Sonarr""#,
        r#""configContract":"SonarrSettings""#,
        r#""name":"prowlarrUrl""#,
        "http://prowlarr:9696",
        r#""name":"baseUrl""#,
        "http://sonarr:8989",
        r#""name":"apiKey""#,
        "arr-key",
        r#""name":"syncCategories""#,
        "5000",
    ] {
        assert!(
            body.contains(expected),
            "Sonarr body missing {expected}: {body}"
        );
    }
    // The television categories, not a movie one.
    assert!(
        !body.contains("2000"),
        "no movie category in a TV sync: {body}"
    );
}

#[tokio::test]
async fn a_radarr_application_carries_its_schema_and_movie_categories() {
    let fake = Fake::always(Answer::reply(201, ""));
    let radarr = application("Radarr", ApplicationKind::Radarr, "http://radarr:7878");
    assert!(prowlarr(&fake).register_application(&radarr).await.is_ok());

    let body = fake
        .request()
        .and_then(|request| request.body)
        .unwrap_or_default();
    for expected in [
        r#""implementation":"Radarr""#,
        r#""configContract":"RadarrSettings""#,
        "2000",
    ] {
        assert!(
            body.contains(expected),
            "Radarr body missing {expected}: {body}"
        );
    }
}

#[tokio::test]
async fn a_lidarr_application_carries_its_schema_and_music_categories() {
    let fake = Fake::always(Answer::reply(201, ""));
    let lidarr = application("Lidarr", ApplicationKind::Lidarr, "http://lidarr:8686");
    assert!(prowlarr(&fake).register_application(&lidarr).await.is_ok());

    let body = fake
        .request()
        .and_then(|request| request.body)
        .unwrap_or_default();
    for expected in [
        r#""implementation":"Lidarr""#,
        r#""configContract":"LidarrSettings""#,
        "3000",
    ] {
        assert!(
            body.contains(expected),
            "Lidarr body missing {expected}: {body}"
        );
    }
}

#[tokio::test]
async fn a_rejected_application_registration_is_refused() {
    let fake = Fake::always(Answer::reply(400, "unknown implementation"));
    let sonarr = application("Sonarr", ApplicationKind::Sonarr, "http://sonarr:8989");
    assert!(matches!(
        prowlarr(&fake).register_application(&sonarr).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_registration_with_no_answer_is_unavailable() {
    let fake = Fake::always(Answer::Silent);
    let sonarr = application("Sonarr", ApplicationKind::Sonarr, "http://sonarr:8989");
    assert!(matches!(
        prowlarr(&fake).register_application(&sonarr).await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn the_applications_are_read_back_by_their_base_url() {
    // Prowlarr carries the connection settings as named entries in a `fields`
    // array, not top-level keys; the address is decoded from there.
    let fake = Fake::always(Answer::reply(
        200,
        r#"[{"id":3,"name":"Sonarr","fields":[{"name":"baseUrl","value":"http://sonarr:8989"},{"name":"apiKey","value":"x"}]}]"#,
    ));
    let applications = prowlarr(&fake).applications().await;
    assert_eq!(
        applications.ok(),
        Some(vec![RegisteredApplication {
            id: "3".to_owned(),
            base_url: "http://sonarr:8989".to_owned(),
        }])
    );

    let sent = fake.request();
    assert!(sent.is_some_and(|request| request.url.ends_with("/api/v1/applications")));
}

#[tokio::test]
async fn an_application_that_names_no_base_url_is_left_out_rather_than_guessed() {
    // A resource without a baseUrl cannot be matched by connection, so it is left
    // out rather than returned as an unusable half-entry.
    let fake = Fake::always(Answer::reply(
        200,
        r#"[{"id":3,"fields":[{"name":"baseUrl","value":"http://sonarr:8989"}]},{"id":4,"fields":[{"name":"apiKey","value":"x"}]}]"#,
    ));
    let applications = prowlarr(&fake)
        .applications()
        .await
        .ok()
        .unwrap_or_default();
    assert_eq!(
        applications.len(),
        1,
        "the entry with no address is left out"
    );
    assert!(applications
        .iter()
        .any(|app| app.id == "3" && app.base_url == "http://sonarr:8989"));
}

#[tokio::test]
async fn an_unreadable_application_list_is_refused() {
    let fake = Fake::always(Answer::reply(200, "not an array"));
    assert!(matches!(
        prowlarr(&fake).applications().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn an_application_listing_that_is_refused_is_unauthorised() {
    let fake = Fake::always(Answer::reply(401, ""));
    assert!(matches!(
        prowlarr(&fake).applications().await,
        Err(Failure::Unauthorised { .. })
    ));
}

#[tokio::test]
async fn an_application_listing_with_no_answer_is_unavailable() {
    let fake = Fake::always(Answer::Silent);
    assert!(matches!(
        prowlarr(&fake).applications().await,
        Err(Failure::Unavailable { .. })
    ));
}

/// Two indexers as Prowlarr lists them, one of them switched off. The `status`
/// field is present and null, which is all Prowlarr ever puts there.
const INDEXERS: &str = r#"[
    {"id":1,"name":"Fast Indexer","enable":true,"status":null},
    {"id":2,"name":"Old Indexer","enable":false,"status":null}
]"#;

/// The standings, from the endpoint that does fill them in.
const STANDINGS: &str = r#"[{"indexerId":2,"disabledTill":"2026-08-16T20:00:00Z"}]"#;

/// The counts for the window asked for. The second indexer is not mentioned,
/// because nothing has been asked of it.
const COUNTS: &str = r#"{"indexers":[
    {"indexerId":1,"numberOfQueries":142,"numberOfGrabs":7,
     "numberOfFailedQueries":3,"numberOfFailedGrabs":1}
]}"#;

/// Reading how much of an allowance has gone must not spend any of it, so every
/// figure comes from Prowlarr's own records rather than from asking the indexers.
#[tokio::test]
async fn indexer_use_is_read_from_the_aggregator_rather_than_from_the_indexers() {
    let fake = Fake::in_turn(vec![
        Answer::reply(200, INDEXERS),
        Answer::reply(200, STANDINGS),
        Answer::reply(200, COUNTS),
    ]);
    let indexers = prowlarr(&fake).indexers(TODAY).await.unwrap_or_default();

    assert_eq!(
        indexers,
        vec![
            IndexerUse {
                name: "Fast Indexer".to_owned(),
                enabled: true,
                queries: 142,
                failed_queries: 3,
                grabs: 7,
                failed_grabs: 1,
                rested_until: None,
            },
            // Nothing has been asked of the second one, which is a zero rather than a
            // gap — and its standing comes from the endpoint that fills one in.
            IndexerUse {
                name: "Old Indexer".to_owned(),
                enabled: false,
                queries: 0,
                failed_queries: 0,
                grabs: 0,
                failed_grabs: 0,
                rested_until: Some("2026-08-16T20:00:00Z".to_owned()),
            },
        ]
    );

    assert!(fake.asked_for("/api/v1/indexerstatus"));
    assert!(fake.asked_for("/api/v1/indexerstats?startDate=2026-08-16T00:00:00"));
}

#[tokio::test]
async fn indexers_that_cannot_be_read_are_a_failure_rather_than_an_empty_list() {
    for answers in [
        vec![Answer::reply(200, "not an array")],
        vec![Answer::reply(200, INDEXERS), Answer::reply(200, "{}")],
        vec![
            Answer::reply(200, INDEXERS),
            Answer::reply(200, STANDINGS),
            Answer::reply(200, "not an object"),
        ],
    ] {
        let fake = Fake::in_turn(answers);
        assert!(matches!(
            prowlarr(&fake).indexers(TODAY).await,
            Err(Failure::Refused { .. })
        ));
    }
}
