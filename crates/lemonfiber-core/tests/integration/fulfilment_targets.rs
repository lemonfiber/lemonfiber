//! Handing the request service the curators that fetch what the household asks for.
//!
//! Driven over the request service's adapter, faked as a transport answering
//! `request.intake`, so these read the requests this product actually sends rather than a
//! second description of them — and from outside the crate, because the app layer is
//! compiled twice and an async path exercised from only one of those leaves the other copy
//! counted as never run.

use lemonfiber_contract::capabilities::request::intake as contract;
use lemonfiber_contract::wire::Kind as Refusal;
use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::media::Kind;
use lemonfiber_core::ports::service::{
    Endpoint, FulfilmentTarget, QualityProfile, RegisteredTarget,
};
use lemonfiber_core::seed::{wire_fulfilment_targets, State};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_testing::adapted::refused;
use lemonfiber_testing::requests::{
    answered, intake, refusing, taken, told, unavailable, REQUESTS,
};
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

/// The curator as the request service lists it, held at `at` under `key`.
fn registered(at: Endpoint, key: &str) -> RegisteredTarget {
    RegisteredTarget {
        id: "1".to_owned(),
        at,
        key: key.to_owned(),
        kind: Kind::Tv,
    }
}

/// The curator held at its own address under the key it should present.
fn held_directly() -> RegisteredTarget {
    registered(curator().at, "the-key")
}

/// The request service over its contract, answering each operation as `routes` say.
fn service(routes: Vec<(&'static str, Vec<Answer>)>) -> (contract::Adapter, Arc<Fake>) {
    let http = Fake::by_path_in_turn(routes);
    (contract::Adapter(REQUESTS.contracted(http.clone())), http)
}

/// A request service listing `listed` on every read, answering a registration, a move
/// and its own test of a target as `written` and `tested` say.
fn holding(
    listed: &[RegisteredTarget],
    written: Answer,
    tested: Answer,
) -> (contract::Adapter, Arc<Fake>) {
    let listed = answered(&listed);
    service(vec![
        (intake("fulfilment_targets"), vec![listed.clone(), listed]),
        (intake("add_fulfilment_target"), vec![written.clone()]),
        (intake("move_fulfilment_target"), vec![written]),
        (intake("test_fulfilment_target"), vec![tested]),
    ])
}

/// A request service listing nothing, then the curator once it is registered.
fn lacking() -> (contract::Adapter, Arc<Fake>) {
    service(vec![
        (
            intake("fulfilment_targets"),
            vec![
                answered(&Vec::<RegisteredTarget>::new()),
                answered(&[held_directly()]),
            ],
        ),
        (intake("add_fulfilment_target"), vec![taken()]),
        (intake("test_fulfilment_target"), vec![taken()]),
    ])
}

async fn passed(
    request_service: &contract::Adapter,
    wanted: &[FulfilmentTarget],
    rehearsing: bool,
) -> Vec<State> {
    let mut journal = Journal::new();
    wire_fulfilment_targets(
        request_service,
        wanted,
        &mut journal,
        "2026-08-28T00:00:00Z",
        rehearsing,
    )
    .await
    .into_iter()
    .map(|wiring| wiring.state)
    .collect()
}

async fn wire(request_service: &contract::Adapter, wanted: &[FulfilmentTarget]) -> Vec<State> {
    passed(request_service, wanted, false).await
}

/// The same pass over the same request service, asked what it would hand over rather
/// than asked to hand it over.
async fn would_wire(
    request_service: &contract::Adapter,
    wanted: &[FulfilmentTarget],
) -> Vec<State> {
    passed(request_service, wanted, true).await
}

/// Whether anything was written to the request service.
fn written(http: &Fake) -> bool {
    http.requests().iter().any(|asked| {
        asked.url.ends_with(intake("add_fulfilment_target"))
            || asked.url.ends_with(intake("move_fulfilment_target"))
    })
}

/// A curator the request service does not know about is handed over, whole.
///
/// The whole point: until it is told, a request is accepted and no downloader ever
/// hears about it. What was sent is read off the request rather than taken on trust.
#[tokio::test]
async fn a_curator_the_request_service_lacks_is_handed_over() {
    let (request_service, http) = lacking();

    let states = wire(&request_service, &[curator()]).await;

    assert_eq!(states, vec![State::Wired], "{states:?}");
    assert_eq!(
        told::<FulfilmentTarget>(&http.requests(), "add_fulfilment_target", "target"),
        vec![curator()]
    );
}

/// One already there is left exactly as it is.
///
/// Matched by where it is reached rather than by name, so an operator who renamed it is
/// not handed a second copy of the same service — and never rewritten, so whatever they
/// changed about it survives a seed.
#[tokio::test]
async fn a_curator_already_registered_is_left_untouched() {
    let (request_service, http) = holding(&[held_directly()], taken(), taken());

    let states = wire(&request_service, &[curator()]).await;

    assert_eq!(states, vec![State::AlreadyWired], "{states:?}");
    assert!(!written(&http), "a target already there was written over");
}

/// A request service that will not answer is skipped, not failed.
#[tokio::test]
async fn a_request_service_that_will_not_answer_is_skipped() {
    let (request_service, _) = service(vec![(intake("fulfilment_targets"), vec![unavailable()])]);

    let states = wire(&request_service, &[curator()]).await;

    assert!(
        matches!(states.first(), Some(State::Skipped { .. })),
        "{states:?}"
    );
}

/// A rehearsal names the endpoint it would register a curator at, and registers none.
///
/// The address is the whole of what the operator is deciding about: the request
/// service reaches a curator over the stack's own network, and a report that said only
/// "a target would be added" would leave them unable to tell a correct run from one
/// about to point the service at the wrong container.
#[tokio::test]
async fn a_rehearsed_pass_names_the_endpoint_it_would_register_and_registers_none() {
    let (request_service, http) = lacking();

    let states = would_wire(&request_service, &[curator()]).await;

    assert_eq!(
        states,
        vec![State::WouldWire {
            yours: None,
            ours: Some("at sonarr:8989".to_owned()),
        }],
        "{states:?}"
    );
    assert!(!written(&http), "a rehearsal handed the curator over");
}

/// A read a rehearsal makes of a request service not yet set up says it could not
/// tell, rather than naming a credential fault nobody has.
#[tokio::test]
async fn a_rehearsed_read_as_the_owner_says_it_could_not_tell_rather_than_naming_a_fault() {
    let (request_service, _) = service(vec![(
        intake("fulfilment_targets"),
        vec![refused(Refusal::Unauthorised)],
    )]);

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

/// A curator held where it was reached before is moved in place, to the gate's endpoint
/// and key, and the request service is asked to test it there.
#[tokio::test]
async fn a_curator_held_where_it_was_reached_before_is_moved_in_place() {
    let (request_service, http) = holding(&[held_directly()], taken(), taken());

    let states = wire(&request_service, &[gated()]).await;

    assert_eq!(states, vec![State::Wired], "{states:?}");
    let sent = http.requests();
    assert_eq!(
        told::<RegisteredTarget>(&sent, "move_fulfilment_target", "held"),
        vec![held_directly()]
    );
    assert_eq!(
        told::<Endpoint>(&sent, "move_fulfilment_target", "at"),
        vec![gated().at]
    );
    assert_eq!(
        told::<String>(&sent, "move_fulfilment_target", "key"),
        vec!["the-token".to_owned()]
    );
    assert_eq!(
        told::<Endpoint>(&sent, "test_fulfilment_target", "at"),
        vec![gated().at]
    );
}

/// A curator held at the right endpoint under another key has its key rewritten.
#[tokio::test]
async fn a_curator_held_under_another_key_has_it_rewritten() {
    let (request_service, http) =
        holding(&[registered(curator().at, "an-old-key")], taken(), taken());

    let states = wire(&request_service, &[curator()]).await;

    assert_eq!(states, vec![State::Wired], "{states:?}");
    assert_eq!(
        told::<String>(&http.requests(), "move_fulfilment_target", "key"),
        vec!["the-key".to_owned()]
    );
}

/// A rehearsal says where the curator would move from and to, and moves nothing.
#[tokio::test]
async fn a_rehearsed_move_says_where_and_moves_nothing() {
    let (request_service, http) = holding(&[held_directly()], taken(), taken());

    let states = would_wire(&request_service, &[gated()]).await;

    assert_eq!(
        states,
        vec![State::WouldWire {
            yours: Some("at sonarr:8989".to_owned()),
            ours: Some("at the request gate".to_owned()),
        }],
        "{states:?}"
    );
    assert!(!written(&http));
}

/// A move or a registration the request service refuses is reported rather than called
/// done.
#[tokio::test]
async fn a_move_or_registration_that_does_not_land_is_reported() {
    let (request_service, _) = holding(&[held_directly()], refusing(), taken());
    let moved = wire(&request_service, &[gated()]).await;

    let (request_service, _) = service(vec![
        (
            intake("fulfilment_targets"),
            vec![answered(&Vec::<RegisteredTarget>::new())],
        ),
        (intake("add_fulfilment_target"), vec![refusing()]),
    ]);
    let added = wire(&request_service, &[curator()]).await;

    for states in [moved, added] {
        assert!(
            matches!(states.first(), Some(State::Failed { .. })),
            "{states:?}"
        );
    }
}

/// A target moved but not reached by the request service's own test is not wired.
#[tokio::test]
async fn a_target_the_request_service_cannot_reach_is_not_wired() {
    let (request_service, _) = holding(&[held_directly()], taken(), unavailable());

    let states = wire(&request_service, &[gated()]).await;

    assert!(
        matches!(states.first(), Some(State::Failed { detail }) if detail.starts_with("the request service's own test of the target failed: ")),
        "{states:?}"
    );
}
