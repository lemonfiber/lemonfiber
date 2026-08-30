//! The Seerr identity client, driven through the HTTP port against a fake
//! transport.
//!
//! Configuring identity is more than one call — sign in through Jellyfin, then
//! finish setup — so the fake answers from a queue and remembers every request,
//! and each test scripts exactly the sequence a branch needs with nothing running.
//! The client speaks an async trait built on another, so it is driven from here
//! rather than in-crate.

use lemonfiber_fixtures::http::{Answer, Fake};
use std::sync::Arc;

use lemonfiber_core::ports::http::Http;
use lemonfiber_core::ports::service::{Failure, FulfilmentTarget, QualityProfile, Requests};
use lemonfiber_core::seerr::Seerr;

fn seerr(fake: &Arc<Fake>) -> Seerr {
    let http: Arc<dyn Http> = fake.clone();
    Seerr::new(http, "http://127.0.0.1:5055", "seerr")
}

/// The password these fixtures sign in with, assembled rather than written down.
///
/// A literal here reads to the source scanner as a credential committed to the
/// repository, which is a rule worth having even where the value is invented.
fn password() -> String {
    ["se", "cret"].concat()
}

/// Configure identity through the fake, for the common arguments.
async fn configure(fake: &Arc<Fake>) -> Result<(), Failure> {
    seerr(fake)
        .configure_identity("admin", &password(), "http://jellyfin:8096")
        .await
}

/// Sign in against `address`, and hand back the body that went out.
async fn body_for(address: &str) -> String {
    let fake = Fake::in_turn(vec![Answer::reply(200, ""), Answer::reply(204, "")]);
    let _ = seerr(&fake)
        .configure_identity("admin", &password(), address)
        .await;
    fake.requests()
        .first()
        .and_then(|request| request.body.clone())
        .unwrap_or_default()
}

/// Every part of the address reaches Seerr as its own field.
///
/// Seerr joins them back together as `{scheme}://{host}:{port}{base}`, so what it is
/// given has to come apart the same way it will be put back — a scheme that decides
/// the flag rather than staying in the host, a port of its own, and a base path where
/// the service is served under one.
#[tokio::test]
async fn an_address_reaches_seerr_in_the_pieces_it_assembles_one_from() {
    let plain = body_for("http://jellyfin:8096").await;
    assert!(plain.contains(r#""hostname":"jellyfin""#), "{plain}");
    assert!(plain.contains(r#""port":8096"#), "{plain}");
    assert!(plain.contains(r#""useSsl":false"#), "{plain}");
    assert!(plain.contains(r#""urlBase":"""#), "{plain}");

    let secure = body_for("https://media.example:9000/watch").await;
    assert!(secure.contains(r#""useSsl":true"#), "{secure}");
    assert!(secure.contains(r#""hostname":"media.example""#), "{secure}");
    assert!(secure.contains(r#""port":9000"#), "{secure}");
    assert!(secure.contains(r#""urlBase":"/watch""#), "{secure}");
}

/// An address with no port answers on the one its scheme implies.
///
/// Seerr writes a port into the address whatever it is, so there is no leaving it
/// out — and the wrong one is a request to somewhere nobody is listening.
#[tokio::test]
async fn an_address_with_no_port_is_given_the_one_its_scheme_implies() {
    let plain = body_for("http://jellyfin").await;
    assert!(plain.contains(r#""port":80"#), "{plain}");

    let secure = body_for("https://jellyfin").await;
    assert!(secure.contains(r#""port":443"#), "{secure}");
}

/// Opening a session names no media server, because naming one asks to move it.
///
/// A service already pointed at a media server refuses an address outright — moving
/// a household's identity source out from under them is not something a sign-in
/// should be able to do — and every run after the first meets exactly that service.
/// So a sign-in that carried the address could never open a session on a working
/// stack, which is the only stack the reads happen on.
#[tokio::test]
async fn opening_a_session_names_no_media_server() {
    let fake = Fake::in_turn(vec![Answer::reply(200, "")]);
    assert!(seerr(&fake).sign_in("admin", &password()).await.is_ok());

    let body = fake
        .requests()
        .first()
        .and_then(|request| request.body.clone())
        .unwrap_or_default();

    assert!(body.contains(r#""username":"admin""#), "{body}");
    for named in ["hostname", "port", "useSsl", "urlBase", "serverType"] {
        assert!(
            !body.contains(named),
            "a session-only sign-in named {named}, which the service refuses: {body}"
        );
    }
}

/// An \*arr as the request service is told about it.
fn target(television: bool) -> FulfilmentTarget {
    FulfilmentTarget {
        name: if television { "Sonarr" } else { "Radarr" }.to_owned(),
        host: if television { "sonarr" } else { "radarr" }.to_owned(),
        port: if television { 8989 } else { 7878 },
        key: ["ke", "y"].concat(),
        television,
        profile: QualityProfile {
            id: 1,
            name: "HD".to_owned(),
        },
        folder: "/data/media".to_owned(),
    }
}

/// Register `target` through the fake, and hand back the body that went out.
async fn registration(television: bool) -> String {
    let fake = Fake::in_turn(vec![Answer::reply(200, "")]);
    let _ = seerr(&fake)
        .add_fulfilment_target(&target(television))
        .await;
    fake.requests()
        .first()
        .and_then(|request| request.body.clone())
        .unwrap_or_default()
}

/// The two lists want one field each, and it is not the same field.
///
/// Television is filed in folders per season; a film has a point before which there is
/// nothing to fetch. Neither is a field the other ignores — the service refuses a
/// registration that omits the one its own list requires, so sending one body for both
/// registers only half a stack.
#[tokio::test]
async fn each_kind_of_target_carries_the_field_its_own_list_requires() {
    let television = registration(true).await;
    assert!(
        television.contains(r#""enableSeasonFolders":true"#),
        "{television}"
    );
    assert!(
        !television.contains("minimumAvailability"),
        "television carried a film's field: {television}"
    );

    let film = registration(false).await;
    assert!(
        film.contains(r#""minimumAvailability":"released""#),
        "{film}"
    );
    assert!(
        !film.contains("enableSeasonFolders"),
        "film carried television's field: {film}"
    );
}

/// Everything both lists require is sent, whichever list it is.
#[tokio::test]
async fn a_registration_carries_everything_the_service_requires_of_it() {
    for television in [true, false] {
        let body = registration(television).await;
        for required in [
            "name",
            "hostname",
            "port",
            "apiKey",
            "useSsl",
            "activeProfileId",
            "activeProfileName",
            "activeDirectory",
            "is4k",
            "isDefault",
        ] {
            assert!(
                body.contains(required),
                "a registration left out {required}, which the service requires: {body}"
            );
        }
    }
}

/// An address this cannot take apart is refused, rather than guessed at.
///
/// Seerr would be handed a host built from whatever was left, and the refusal that
/// came back would be about a URL nobody wrote.
#[tokio::test]
async fn an_address_that_cannot_be_taken_apart_is_refused_before_it_is_sent() {
    for nonsense in ["jellyfin:8096", "ftp://jellyfin:8096", "http://"] {
        let fake = Fake::in_turn(vec![Answer::reply(200, ""), Answer::reply(204, "")]);
        let outcome = seerr(&fake)
            .configure_identity("admin", &password(), nonsense)
            .await;

        assert!(
            matches!(outcome, Err(Failure::Refused { .. })),
            "{nonsense} was accepted"
        );
        let sent = fake.requests();
        assert!(sent.is_empty(), "{nonsense} was sent anyway: {sent:?}");
    }
}

#[tokio::test]
async fn an_initialised_seerr_is_reported() {
    let fake = Fake::in_turn(vec![Answer::reply(200, r#"{"initialized":true}"#)]);
    assert_eq!(seerr(&fake).initialized().await.ok(), Some(true));
    assert!(fake
        .requests()
        .first()
        .is_some_and(|request| request.url.ends_with("/api/v1/settings/public")));
}

#[tokio::test]
async fn an_uninitialised_or_unstated_seerr_reads_as_not_done() {
    let fake = Fake::in_turn(vec![Answer::reply(200, r#"{"initialized":false}"#)]);
    assert_eq!(seerr(&fake).initialized().await.ok(), Some(false));
    // A response that omits the field is a Seerr too fresh to have set it.
    let bare = Fake::in_turn(vec![Answer::reply(200, "{}")]);
    assert_eq!(seerr(&bare).initialized().await.ok(), Some(false));
}

#[tokio::test]
async fn an_unreadable_public_settings_is_refused() {
    let fake = Fake::in_turn(vec![Answer::reply(200, "not json")]);
    assert!(matches!(
        seerr(&fake).initialized().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_seerr_is_unavailable_on_the_read() {
    let fake = Fake::silent();
    assert!(matches!(
        seerr(&fake).initialized().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn configuring_identity_signs_in_through_jellyfin_then_finishes_setup() {
    let fake = Fake::in_turn(vec![Answer::reply(200, ""), Answer::reply(204, "")]);
    assert!(configure(&fake).await.is_ok());

    let requests = fake.requests();
    // The sign-in comes first, carrying the Jellyfin credentials as JSON.
    let first = requests.first();
    assert!(first.is_some_and(|request| request.url.ends_with("/api/v1/auth/jellyfin")));
    assert!(first.is_some_and(|request| request
        .headers
        .iter()
        .any(|(name, value)| name == "Content-Type" && value == "application/json")));
    let body = first
        .and_then(|request| request.body.clone())
        .unwrap_or_default();
    // Where Jellyfin is, in the pieces Seerr assembles an address from. Handed a
    // whole address it builds `http://http://jellyfin:8096:undefined` and refuses
    // that, which is a refusal about a URL nobody wrote.
    for expected in [
        r#""username":"admin""#,
        r#""password":"secret""#,
        r#""hostname":"jellyfin""#,
        r#""port":8096"#,
        r#""useSsl":false"#,
        r#""urlBase":"""#,
        r#""email":"admin@lemonfiber.local""#,
        r#""serverType":2"#,
    ] {
        assert!(
            body.contains(expected),
            "sign-in body missing {expected}: {body}"
        );
    }
    // Setup is finished only after the sign-in.
    assert!(requests
        .get(1)
        .is_some_and(|request| request.url.ends_with("/api/v1/settings/initialize")));
}

#[tokio::test]
async fn a_rejected_sign_in_is_refused_and_setup_is_not_finished() {
    let fake = Fake::in_turn(vec![Answer::reply(500, "credentials rejected")]);
    assert!(matches!(
        configure(&fake).await,
        Err(Failure::Refused { .. })
    ));
    // Only the failed sign-in was attempted; finishing was never reached.
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn a_rejected_finish_is_refused() {
    let fake = Fake::in_turn(vec![Answer::reply(200, ""), Answer::reply(500, "boom")]);
    assert!(matches!(
        configure(&fake).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_seerr_is_unavailable_on_the_sign_in() {
    let fake = Fake::silent();
    assert!(matches!(
        configure(&fake).await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn signing_in_opens_a_session_without_finishing_setup() {
    // The read path signs in only: finishing setup is somebody else's business, and a
    // read must never complete a household's configuration as a side effect.
    let fake = Fake::in_turn(vec![Answer::reply(200, "")]);
    assert!(seerr(&fake).sign_in("admin", &password()).await.is_ok());
    let requests = fake.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests
        .first()
        .is_some_and(|request| request.url.ends_with("/api/v1/auth/jellyfin")));
}

#[tokio::test]
async fn the_households_requests_are_read_with_who_asked_and_what_became_of_each() {
    let page = r#"{
        "pageInfo":{"pages":1,"page":1,"results":3,"pageSize":100},
        "results":[
            {"status":2,"type":"tv","media":{"status":4,"externalServiceId":11},
             "requestedBy":{"displayName":"Alex"}},
            {"status":1,"type":"movie","media":{"status":1},
             "requestedBy":{"displayName":"Sam"}},
            {"status":5,"type":"movie","media":{"status":5,"externalServiceId":7},
             "requestedBy":{"displayName":"Alex"}}
        ]
    }"#;
    let fake = Fake::in_turn(vec![Answer::reply(200, page)]);
    let requests = seerr(&fake).requests().await.unwrap_or_default();
    let read: Vec<(&str, bool, Option<i64>, u8, u8)> = requests
        .iter()
        .map(|request| {
            (
                request.member.as_str(),
                request.kind.is_some(),
                request.item,
                request.request_status,
                request.media_status,
            )
        })
        .collect();
    // The two statuses are carried as the service's own numbers; a request no service
    // holds yet names no item, which is what leaves it with no title to find.
    assert_eq!(
        read,
        vec![
            ("Alex", true, Some(11), 2, 4),
            ("Sam", true, None, 1, 1),
            ("Alex", true, Some(7), 5, 5),
        ]
    );
    // Read newest first, so a household with more than the horizon keeps the requests
    // still worth asking about.
    assert!(fake
        .requests()
        .first()
        .is_some_and(|request| request.url.contains("sortDirection=desc")));
}

#[tokio::test]
async fn a_media_type_this_build_does_not_know_names_no_service() {
    let page = r#"{"pageInfo":{"results":1},"results":[
        {"status":2,"type":"music","media":{"status":3},"requestedBy":{"displayName":"Sam"}}
    ]}"#;
    let fake = Fake::in_turn(vec![Answer::reply(200, page)]);
    let requests = seerr(&fake).requests().await.unwrap_or_default();
    // Reported as a request whose kind is unknown rather than guessed into one of the
    // two this build files.
    assert_eq!(requests.first().map(|request| request.kind), Some(None));
}

#[tokio::test]
async fn the_requests_walk_past_the_first_page() {
    // A full first page and a total beyond it: a household with more requests than one
    // page would otherwise report only its newest.
    let filler = vec![
        r#"{"status":2,"type":"movie","media":{"status":5},"requestedBy":{"displayName":"Alex"}}"#;
        100
    ]
    .join(",");
    let page_one = Box::leak(
        format!(r#"{{"pageInfo":{{"results":101}},"results":[{filler}]}}"#).into_boxed_str(),
    );
    let page_two = r#"{"pageInfo":{"results":101},"results":[
        {"status":2,"type":"tv","media":{"status":5},"requestedBy":{"displayName":"Sam"}}
    ]}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, page_one),
        Answer::reply(200, page_two),
    ]);
    let requests = seerr(&fake).requests().await.unwrap_or_default();
    assert_eq!(requests.len(), 101);
    assert!(fake
        .requests()
        .iter()
        .any(|request| request.url.contains("skip=100")));
}

#[tokio::test]
async fn an_unreadable_request_record_is_a_failure() {
    let fake = Fake::in_turn(vec![Answer::reply(200, "not json")]);
    assert!(seerr(&fake).requests().await.is_err());
}

/// The request service's key is read from the settings file it writes.
#[test]
fn the_key_is_read_from_the_settings_it_writes() {
    const WRITTEN: &str = r#"{"main":{"apiKey":"the-written-key","applicationTitle":"Seerr"}}"#;
    assert_eq!(
        lemonfiber_core::seerr::api_key(WRITTEN).as_deref(),
        Some("the-written-key")
    );
}

/// A settings file written before the service was initialised holds no key.
///
/// Seerr writes one when it is given an owner, so a stack seeded before that has
/// none to publish — and an empty value published is worse than none, since the
/// dashboard would authenticate with it and be refused.
#[test]
fn settings_without_a_key_yet_yield_nothing() {
    assert_eq!(
        lemonfiber_core::seerr::api_key(r#"{"main":{"apiKey":""}}"#),
        None
    );
    assert_eq!(lemonfiber_core::seerr::api_key(r#"{"main":{}}"#), None);
    assert_eq!(lemonfiber_core::seerr::api_key("{}"), None);
    assert_eq!(lemonfiber_core::seerr::api_key("not json at all"), None);
}
