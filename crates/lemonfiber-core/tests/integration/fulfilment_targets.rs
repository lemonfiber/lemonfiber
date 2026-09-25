//! Handing the request service the \*arrs that fetch what the household asks for.
//!
//! Driven through the real clients over a scripted stack, so these read the requests
//! this product actually sends rather than a second description of them — and from
//! outside the crate, because the app layer is compiled twice and an async path
//! exercised from only one of those leaves the other copy counted as never run.

use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::service::{
    Client as _, FulfilmentTarget, QualityProfile, Requests as _,
};
use lemonfiber_core::seed::{wire_fulfilment_targets, State};
use lemonfiber_core::seerr::Seerr;
use lemonfiber_core::servarr::Servarr;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use std::sync::Arc;

/// Sonarr as the request service should be told about it.
fn sonarr() -> FulfilmentTarget {
    FulfilmentTarget {
        name: "Sonarr".to_owned(),
        host: "sonarr".to_owned(),
        port: 8989,
        key: "the-key".to_owned(),
        television: true,
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

/// Sonarr as the request service reports it once registered.
fn registered() -> String {
    r#"{"id":1,"hostname":"sonarr","port":8989}"#.to_owned()
}

async fn wire(seerr: &Seerr, wanted: &[FulfilmentTarget]) -> Vec<State> {
    let mut journal = Journal::new();
    wire_fulfilment_targets(seerr, wanted, &mut journal, "2026-08-28T00:00:00Z", false)
        .await
        .into_iter()
        .map(|wiring| wiring.state)
        .collect()
}

/// The same pass over the same request service, asked what it would hand over rather
/// than asked to hand it over.
async fn would_wire(seerr: &Seerr, wanted: &[FulfilmentTarget]) -> Vec<State> {
    let mut journal = Journal::new();
    wire_fulfilment_targets(seerr, wanted, &mut journal, "2026-08-28T00:00:00Z", true)
        .await
        .into_iter()
        .map(|wiring| wiring.state)
        .collect()
}

/// An \*arr the request service does not know about is handed over.
///
/// The whole point: until it is told, a request is accepted and no downloader ever
/// hears about it. What was sent is read off the request rather than taken on trust.
#[tokio::test]
async fn an_arr_the_request_service_lacks_is_handed_over() {
    let (seerr, http) = holding("[]", "[]");

    let states = wire(&seerr, &[sonarr()]).await;

    assert_eq!(states, vec![State::Wired], "{states:?}");
    let sent = http
        .requests()
        .into_iter()
        .find(|asked| asked.method == Method::Post)
        .and_then(|asked| asked.body)
        .unwrap_or_default();
    assert!(
        sent.contains("\"hostname\":\"sonarr\"") && sent.contains("\"port\":8989"),
        "the *arr was not named by the endpoint the request service reaches it on: {sent}"
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
async fn an_arr_already_registered_is_left_untouched_despite_a_different_name() {
    let renamed = format!("[{}]", r#"{"id":1,"hostname":"sonarr","port":8989}"#);
    let (seerr, http) = holding("[]", &renamed);

    let states = wire(&seerr, &[sonarr()]).await;

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
    let seerr = Seerr::new(http, "http://seerr:5055", "seerr");

    let states = wire(&seerr, &[sonarr()]).await;

    assert!(
        matches!(states.first(), Some(State::Skipped { .. })),
        "{states:?}"
    );
}

/// The profiles an \*arr reports, as the request service needs them named.
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
    let arr = Servarr::new(
        http,
        "http://sonarr:8989",
        "the-key".to_owned(),
        "sonarr",
        3,
    );

    let profiles = arr.quality_profiles().await.unwrap_or_default();

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
    let (seerr, _) = holding(
        r#"[{"id":2,"hostname":"radarr","port":7878}]"#,
        &format!("[{}]", registered()),
    );

    let held = seerr.fulfilment_targets().await.unwrap_or_default();

    assert_eq!(held.len(), 2, "both lists were not read: {held:?}");
    assert!(
        held.iter().any(|target| target.television) && held.iter().any(|target| !target.television),
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
    let (seerr, _) = holding("not a list of targets", &format!("[{}]", registered()));

    let said = seerr
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

/// A rehearsal names the endpoint it would register an \*arr at, and registers none.
///
/// The address is the whole of what the operator is deciding about: the request
/// service reaches an \*arr over the stack's own network, and a report that said only
/// "a target would be added" would leave them unable to tell a correct run from one
/// about to point the service at the wrong container. Asserted together with the
/// traffic, because a report that reads right while the write still goes out is the
/// one failure this flag exists to prevent wearing the right words.
#[tokio::test]
async fn a_rehearsed_pass_names_the_endpoint_it_would_register_and_registers_none() {
    let (seerr, http) = holding("[]", "[]");

    let states = would_wire(&seerr, &[sonarr()]).await;

    assert_eq!(
        states,
        vec![State::WouldWire {
            yours: None,
            ours: Some("sonarr:8989".to_owned()),
        }],
        "{states:?}"
    );
    assert!(
        !http
            .requests()
            .iter()
            .any(|asked| asked.method == Method::Post),
        "a rehearsal handed the *arr over"
    );
}

/// A read a rehearsal makes as the owner says it could not tell, rather than naming a
/// credential fault nobody has.
///
/// Every call here is authenticated and nothing but signing in opens a session — and
/// signing in is a `POST` that leaves state on somebody else's service, which a run
/// promising to leave none does not make. So the answer comes back unauthorised, and
/// an operator told their key was refused would go looking for a broken credential and
/// find a working one. What they are told instead is that this run declined to open a
/// session and that a real one signs in and reports what it found.
#[tokio::test]
async fn a_rehearsed_read_as_the_owner_says_it_could_not_tell_rather_than_naming_a_fault() {
    let http = Fake::by_path(vec![("/settings", Answer::reply(401, ""))]);
    let seerr = Seerr::new(http, "http://seerr:5055", "seerr");

    let states = would_wire(&seerr, &[sonarr()]).await;

    let said = format!("{states:?}");
    assert!(
        said.starts_with("[Skipped"),
        "a session this run chose not to open was reported as something else: {said}"
    );
    assert!(
        said.contains("session"),
        "the operator was not told why nothing could be read: {said}"
    );
    assert!(
        !said.contains("credential") && !said.contains("refused"),
        "a rehearsal put a credential fault in front of an operator who has none: {said}"
    );
}
