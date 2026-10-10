//! Handing the request service the curators that fetch what the household asks for.
//!
//! Driven through the real clients over a scripted stack, so these read the requests
//! this product actually sends rather than a second description of them — and from
//! outside the crate, because the app layer is compiled twice and an async path
//! exercised from only one of those leaves the other copy counted as never run.

use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::media::Kind;
use lemonfiber_core::ports::service::{
    Client as _, Endpoint, FulfilmentTarget, QualityProfile, Requests as _,
};
use lemonfiber_core::seed::{wire_fulfilment_targets, State};
use lemonfiber_core::seerr::Seerr;
use lemonfiber_core::servarr::Servarr;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use std::sync::Arc;

/// The curator as the request service should be told about it.
fn curator() -> FulfilmentTarget {
    FulfilmentTarget {
        name: "Sonarr".to_owned(),
        at: Endpoint {
            host: "sonarr".to_owned(),
            port: 8989,
            base: String::new(),
        },
        moved_from: None,
        key: "the-key".to_owned(),
        kind: Kind::Tv,
        profile: QualityProfile {
            id: 4,
            name: "HD-1080p".to_owned(),
        },
        folder: "/data/media/tv".to_owned(),
    }
}

/// A request service holding the given film and television targets.
fn holding(film: &str, television: &str) -> (Seerr, Arc<Fake>) {
    // The read-back after a write reads *both* lists again, so the film list needs a
    // second answer even where nothing was written to it.
    let http = Fake::by_path_in_turn(vec![
        (
            "/settings/radarr",
            vec![
                Answer::reply(200, film.to_owned()),
                Answer::reply(200, film.to_owned()),
            ],
        ),
        (
            "/settings/sonarr",
            vec![
                Answer::reply(200, television.to_owned()),
                Answer::reply(201, String::new()),
                Answer::reply(200, format!("[{}]", registered())),
            ],
        ),
    ]);
    (Seerr::new(http.clone(), "http://seerr:5055", "seerr"), http)
}

/// The curator as the request service reports it once registered.
fn registered() -> String {
    r#"{"id":1,"hostname":"sonarr","port":8989,"apiKey":"the-key"}"#.to_owned()
}

async fn wire(request_service: &Seerr, wanted: &[FulfilmentTarget]) -> Vec<State> {
    let mut journal = Journal::new();
    wire_fulfilment_targets(
        request_service,
        wanted,
        &mut journal,
        "2026-08-28T00:00:00Z",
        false,
    )
    .await
    .into_iter()
    .map(|wiring| wiring.state)
    .collect()
}

/// The same pass over the same request service, asked what it would hand over rather
/// than asked to hand it over.
async fn would_wire(request_service: &Seerr, wanted: &[FulfilmentTarget]) -> Vec<State> {
    let mut journal = Journal::new();
    wire_fulfilment_targets(
        request_service,
        wanted,
        &mut journal,
        "2026-08-28T00:00:00Z",
        true,
    )
    .await
    .into_iter()
    .map(|wiring| wiring.state)
    .collect()
}

/// A curator the request service does not know about is handed over.
///
/// The whole point: until it is told, a request is accepted and no downloader ever
/// hears about it. What was sent is read off the request rather than taken on trust.
#[tokio::test]
async fn a_curator_the_request_service_lacks_is_handed_over() {
    let (request_service, http) = holding("[]", "[]");

    let states = wire(&request_service, &[curator()]).await;

    assert_eq!(states, vec![State::Wired], "{states:?}");
    let sent = http
        .requests()
        .into_iter()
        .find(|asked| asked.method == Method::Post)
        .and_then(|asked| asked.body)
        .unwrap_or_default();
    assert!(
        sent.contains("\"hostname\":\"sonarr\"") && sent.contains("\"port\":8989"),
        "the curator was not named by the endpoint the request service reaches it on: {sent}"
    );
    assert!(
        sent.contains("\"activeProfileId\":4") && sent.contains("HD-1080p"),
        "the request service was given no profile to fetch at: {sent}"
    );
    assert!(
        sent.contains("/data/media/tv"),
        "the request service was given nowhere to file what it fetches: {sent}"
    );
}

/// One already there is left exactly as it is.
///
/// Matched by host and port rather than by name, so an operator who renamed it is
/// not handed a second copy of the same service — and never rewritten, so whatever
/// they changed about it survives a seed.
#[tokio::test]
async fn a_curator_already_registered_is_left_untouched_despite_a_different_name() {
    let renamed = format!(
        "[{}]",
        r#"{"id":1,"name":"Renamed","hostname":"sonarr","port":8989,"apiKey":"the-key"}"#
    );
    let (request_service, http) = holding("[]", &renamed);

    let states = wire(&request_service, &[curator()]).await;

    assert_eq!(states, vec![State::AlreadyWired], "{states:?}");
    assert!(
        !http
            .requests()
            .iter()
            .any(|asked| asked.method == Method::Post),
        "a target already there was written over"
    );
}

/// A request service that will not answer is skipped, not failed.
#[tokio::test]
async fn a_request_service_that_will_not_answer_is_skipped() {
    let http = Fake::by_path_in_turn(vec![("/settings", vec![Answer::Silent])]);
    let request_service = Seerr::new(http, "http://seerr:5055", "seerr");

    let states = wire(&request_service, &[curator()]).await;

    assert!(
        matches!(states.first(), Some(State::Skipped { .. })),
        "{states:?}"
    );
}

/// The profiles a curator reports, as the request service needs them named.
///
/// A profile with no usable id or no name is passed over: the request service must
/// name both when it hands over a request, and half of one is not an answer.
#[tokio::test]
async fn only_profiles_that_can_be_named_are_offered() {
    let listed = r#"[
        {"id":4,"name":"HD-1080p"},
        {"id":-1,"name":"Impossible"},
        {"id":9,"name":""}
    ]"#;
    let http = Fake::by_path_in_turn(vec![("/qualityprofile", vec![Answer::reply(200, listed)])]);
    let curator = Servarr::new(
        http,
        "http://sonarr:8989",
        "the-key".to_owned(),
        "sonarr",
        3,
    );

    let profiles = curator.quality_profiles().await.unwrap_or_default();

    assert_eq!(
        profiles,
        vec![QualityProfile {
            id: 4,
            name: "HD-1080p".to_owned()
        }],
        "a profile the request service could not name was offered anyway"
    );
}

/// What the request service already holds, read back by endpoint.
#[tokio::test]
async fn the_targets_it_holds_are_read_from_both_lists() {
    let (request_service, _) = holding(
        r#"[{"id":2,"hostname":"radarr","port":7878}]"#,
        &format!("[{}]", registered()),
    );

    let held = request_service
        .fulfilment_targets()
        .await
        .unwrap_or_default();

    assert_eq!(held.len(), 2, "both lists were not read: {held:?}");
    assert!(
        held.iter().any(|target| target.kind == Kind::Tv)
            && held.iter().any(|target| target.kind == Kind::Movies),
        "film and television were not told apart: {held:?}"
    );
}

/// A list that will not read back is refused, rather than taken for an empty one.
///
/// The read is what tells this run which targets are already registered, and an
/// unreadable answer treated as "none" is the one mistake that cannot be undone from
/// here: every target is registered again on top of the ones already there, and the
/// household's requests are then fulfilled twice.
#[tokio::test]
async fn a_target_list_that_will_not_read_back_is_refused_rather_than_taken_as_empty() {
    let (request_service, _) = holding("not a list of targets", &format!("[{}]", registered()));

    let said = request_service
        .fulfilment_targets()
        .await
        .err()
        .map(|failure| failure.to_string())
        .unwrap_or_default();

    assert!(
        said.contains("fulfilment targets could not be read"),
        "an unreadable list did not say so: {said}"
    );
}

/// A rehearsal names the endpoint it would register a curator at, and registers none.
///
/// The address is the whole of what the operator is deciding about: the request
/// service reaches a curator over the stack's own network, and a report that said only
/// "a target would be added" would leave them unable to tell a correct run from one
/// about to point the service at the wrong container. Asserted together with the
/// traffic, because a report that reads right while the write still goes out is the
/// one failure this flag exists to prevent wearing the right words.
#[tokio::test]
async fn a_rehearsed_pass_names_the_endpoint_it_would_register_and_registers_none() {
    let (request_service, http) = holding("[]", "[]");

    let states = would_wire(&request_service, &[curator()]).await;

    assert_eq!(
        states,
        vec![State::WouldWire {
            yours: None,
            ours: Some("at sonarr:8989".to_owned()),
        }],
        "{states:?}"
    );
    assert!(
        !http
            .requests()
            .iter()
            .any(|asked| asked.method == Method::Post),
        "a rehearsal handed the curator over"
    );
}

/// A read a rehearsal makes of a request service not yet set up says it could not
/// tell, rather than naming a credential fault nobody has.
///
/// A service not yet set up has written no key, so the client a rehearsal reads with
/// carries none and the answer comes back unauthorised. An operator told their key was
/// refused would go looking for a broken credential that does not exist yet. What they
/// are told instead is that the service is not set up, and that a real run sets it up
/// and reports what it found.
#[tokio::test]
async fn a_rehearsed_read_as_the_owner_says_it_could_not_tell_rather_than_naming_a_fault() {
    let http = Fake::by_path(vec![("/settings", Answer::reply(401, ""))]);
    let request_service = Seerr::new(http, "http://seerr:5055", "seerr");

    let states = would_wire(&request_service, &[curator()]).await;

    let said = format!("{states:?}");
    assert!(
        said.starts_with("[Skipped"),
        "a service not yet set up was reported as something else: {said}"
    );
    assert!(
        said.contains("not been set up"),
        "the operator was not told why nothing could be read: {said}"
    );
    assert!(
        !said.contains("credential") && !said.contains("refused"),
        "a rehearsal put a credential fault in front of an operator who has none: {said}"
    );
}

/// The curator as the request service should reach it through the gate, held before at its
/// own address.
fn gated() -> FulfilmentTarget {
    FulfilmentTarget {
        at: Endpoint {
            host: "request-gate".to_owned(),
            port: 5057,
            base: "/sonarr".to_owned(),
        },
        moved_from: Some(curator().at),
        key: "the-token".to_owned(),
        ..curator()
    }
}

/// A request service listing `listed` for the curator on every read, and answering a move
/// with `moved`.
fn moving(listed: &str, moved: u16) -> (Seerr, Arc<Fake>) {
    moving_tested(listed, moved, 200)
}

/// [`moving`], answering its own test of a target with `tested`.
fn moving_tested(listed: &str, moved: u16, tested: u16) -> (Seerr, Arc<Fake>) {
    let http = Fake::by_route_in_turn(vec![
        (
            Method::Post,
            "/settings/sonarr/test",
            vec![Answer::reply(tested, String::new())],
        ),
        (
            Method::Get,
            "/settings/radarr",
            vec![Answer::reply(200, "[]")],
        ),
        (
            Method::Get,
            "/settings/sonarr",
            vec![Answer::reply(200, listed.to_owned())],
        ),
        (
            Method::Put,
            "/settings/sonarr/1",
            vec![Answer::reply(moved, String::new())],
        ),
    ]);
    (Seerr::new(http.clone(), "http://seerr:5055", "seerr"), http)
}

/// The curator held at its own address, named and profiled by the operator.
const HELD_DIRECTLY: &str = r#"[{"id":1,"name":"Mine","hostname":"sonarr","port":8989,"apiKey":"the-key","activeProfileId":9}]"#;

/// A curator held where it was reached before is moved in place: its endpoint and key
/// change, and everything the operator chose about it stays.
#[tokio::test]
async fn a_curator_held_where_it_was_reached_before_is_moved_in_place() {
    let (request_service, http) = moving(HELD_DIRECTLY, 200);

    let states = wire(&request_service, &[gated()]).await;

    assert_eq!(states, vec![State::Wired], "{states:?}");
    let sent = http
        .requests()
        .into_iter()
        .find(|asked| asked.method == Method::Put)
        .and_then(|asked| asked.body)
        .unwrap_or_default();
    for field in [
        "\"hostname\":\"request-gate\"",
        "\"port\":5057",
        "\"baseUrl\":\"/sonarr\"",
        "\"apiKey\":\"the-token\"",
        "\"name\":\"Mine\"",
        "\"activeProfileId\":9",
    ] {
        assert!(sent.contains(field), "{field} missing from {sent}");
    }
}

/// A curator held at the right endpoint under another key has its key rewritten.
#[tokio::test]
async fn a_curator_held_under_another_key_has_it_rewritten() {
    let (request_service, http) = moving(
        r#"[{"id":1,"hostname":"sonarr","port":8989,"apiKey":"an-old-key"}]"#,
        200,
    );

    let states = wire(&request_service, &[curator()]).await;

    assert_eq!(states, vec![State::Wired], "{states:?}");
    assert!(http
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Put
            && asked
                .body
                .as_deref()
                .is_some_and(|body| body.contains("the-key"))));
}

/// A rehearsal says where the curator would move from and to, and moves nothing.
#[tokio::test]
async fn a_rehearsed_move_says_where_and_moves_nothing() {
    let (request_service, http) = moving(HELD_DIRECTLY, 200);

    let states = would_wire(&request_service, &[gated()]).await;

    assert_eq!(
        states,
        vec![State::WouldWire {
            yours: Some("at sonarr:8989".to_owned()),
            ours: Some("at the request gate".to_owned()),
        }],
        "{states:?}"
    );
    assert!(!http
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Put));
}

/// A move the request service refuses, or one of a target it no longer holds, is
/// reported rather than called done.
#[tokio::test]
async fn a_move_that_does_not_land_is_reported() {
    let (request_service, _) = moving(HELD_DIRECTLY, 500);
    let states = wire(&request_service, &[gated()]).await;
    assert!(
        matches!(states.first(), Some(State::Failed { .. })),
        "{states:?}"
    );

    let (request_service, _) = moving(HELD_DIRECTLY, 200);
    let gone = lemonfiber_core::ports::service::RegisteredTarget {
        id: "7".to_owned(),
        at: curator().at,
        key: String::new(),
        kind: Kind::Tv,
    };
    let said = request_service
        .move_fulfilment_target(&gone, &gated().at, "the-token")
        .await
        .err()
        .map(|failure| failure.to_string())
        .unwrap_or_default();
    assert!(
        said.contains("no longer holds the target it listed as 7"),
        "{said}"
    );
}

/// A film target moves within the film list.
#[tokio::test]
async fn a_film_target_moves_within_the_film_list() {
    let http = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/settings/radarr",
            vec![Answer::reply(
                200,
                r#"[{"id":1,"hostname":"radarr","port":7878,"apiKey":"old"}]"#,
            )],
        ),
        (
            Method::Put,
            "/settings/radarr/1",
            vec![Answer::reply(200, "")],
        ),
    ]);
    let request_service = Seerr::new(http.clone(), "http://seerr:5055", "seerr");
    let held = lemonfiber_core::ports::service::RegisteredTarget {
        id: "1".to_owned(),
        at: Endpoint {
            host: "radarr".to_owned(),
            port: 7878,
            base: String::new(),
        },
        key: "old".to_owned(),
        kind: Kind::Movies,
    };
    let film = FulfilmentTarget {
        kind: Kind::Movies,
        ..gated()
    };

    let moved = request_service
        .move_fulfilment_target(&held, &film.at, &film.key)
        .await;

    assert!(moved.is_ok(), "{moved:?}");
    assert!(http
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Put && asked.url.contains("/settings/radarr/1")));
}

/// A request service that stops answering, or answers with something other than a
/// list, at any point of a move or a registration fails it rather than calling it done.
#[tokio::test]
async fn a_move_or_registration_the_service_does_not_answer_fails() {
    let held = lemonfiber_core::ports::service::RegisteredTarget {
        id: "1".to_owned(),
        at: curator().at,
        key: "the-key".to_owned(),
        kind: Kind::Tv,
    };
    for (name, listed, put) in [
        ("unlisted", Answer::Silent, Answer::reply(200, "")),
        (
            "unreadable",
            Answer::reply(200, "not a list"),
            Answer::reply(200, ""),
        ),
        (
            "unwritten",
            Answer::reply(200, HELD_DIRECTLY),
            Answer::Silent,
        ),
    ] {
        let http = Fake::by_route_in_turn(vec![
            (Method::Get, "/settings/sonarr", vec![listed]),
            (Method::Put, "/settings/sonarr/1", vec![put]),
        ]);
        let request_service = Seerr::new(http, "http://seerr:5055", "seerr");

        assert!(
            request_service
                .move_fulfilment_target(&held, &gated().at, "the-token")
                .await
                .is_err(),
            "{name}"
        );
    }

    let http = Fake::by_route_in_turn(vec![(
        Method::Post,
        "/settings/sonarr",
        vec![Answer::Silent],
    )]);
    let request_service = Seerr::new(http, "http://seerr:5055", "seerr");
    assert!(request_service
        .add_fulfilment_target(&curator())
        .await
        .is_err());
}

/// A target moved but not reached by the request service's own test is not wired.
#[tokio::test]
async fn a_target_the_request_service_cannot_reach_is_not_wired() {
    let (request_service, http) = moving_tested(HELD_DIRECTLY, 200, 500);

    let states = wire(&request_service, &[gated()]).await;

    assert!(
        matches!(states.first(), Some(State::Failed { detail }) if detail.starts_with("the request service's own test of the target failed: ")),
        "{states:?}"
    );
    let tested = http
        .requests()
        .into_iter()
        .find(|asked| asked.url.ends_with("/settings/sonarr/test"))
        .and_then(|asked| asked.body)
        .unwrap_or_default();
    assert!(
        tested.contains("\"hostname\":\"request-gate\"")
            && tested.contains("\"baseUrl\":\"/sonarr\"")
            && tested.contains("\"apiKey\":\"the-token\""),
        "{tested}"
    );
}
