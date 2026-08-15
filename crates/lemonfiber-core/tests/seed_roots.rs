//! Wiring root folders into a \\*arr, against a fake service.
//!
//! The first driver, and the one the others are shaped after: observe, write only
//! what is missing, read it back before calling it wired, and journal the change.

mod common;

use common::service::*;
use std::collections::BTreeMap;

use lemonfiber_core::ports::service::RegisteredFolder;
use lemonfiber_core::seed::{contested_roots, State};

#[tokio::test]
async fn a_write_that_landed_before_an_interruption_is_not_duplicated_on_the_next_run() {
    // The load-bearing interruption: the write reached the service, but the run died
    // before the read-back could confirm it. `DropsAfterRegister` is exactly that —
    // the folder is registered (and kept by the service), then the confirming read
    // fails, so the pass leaves it outstanding rather than calling it done.
    let interrupted = FakeService::with(Mode::DropsAfterRegister, Vec::new());
    let first = wire_on(&interrupted, &[folder("/data/media/tv")]).await;
    assert!(
        matches!(first.as_slice(), [State::Skipped { .. }]),
        "an unconfirmed write is left outstanding, not called done: {first:?}"
    );

    // The service kept the folder the interrupted run registered — the state a
    // killed run leaves behind. The next run, now answering, must find it already
    // there and leave it, not register a second copy.
    let landed = interrupted.registered();
    assert_eq!(landed.len(), 1, "the write did land at the service");
    let (states, wrote) = seed(
        FakeService::with(Mode::Normal, landed),
        &[folder("/data/media/tv")],
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(
        wrote, 0,
        "the connection that survived the interruption is left intact, not duplicated"
    );
}

#[tokio::test]
async fn an_absent_folder_is_registered_read_back_and_recorded() {
    let (states, recorded) = seed(
        FakeService::with(Mode::Normal, Vec::new()),
        &[folder("/data/media/tv")],
    )
    .await;
    assert_eq!(states, vec![State::Wired]);
    assert_eq!(recorded, 1, "the write is journalled so it can be undone");
}

#[tokio::test]
async fn a_folder_the_service_already_has_is_left_untouched() {
    // Idempotent: the folder is present, so nothing is written or journalled.
    let existing = vec![RegisteredFolder {
        id: "1".to_owned(),
        path: "/data/media/tv".to_owned(),
    }];
    let (states, recorded) = seed(
        FakeService::with(Mode::Normal, existing),
        &[folder("/data/media/tv")],
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(recorded, 0, "an already-wired connection writes nothing");
}

#[tokio::test]
async fn a_wanted_path_is_matched_to_the_services_canonical_form() {
    // The service stores the path without its trailing slash. The read-back must
    // still recognise the folder it just registered, wire it, and record it —
    // rather than declaring the landed write a failure.
    let (states, recorded) = seed(
        FakeService::with(Mode::Normal, Vec::new()),
        &[folder("/data/media/tv/")],
    )
    .await;
    assert_eq!(states, vec![State::Wired]);
    assert_eq!(recorded, 1);
}

#[tokio::test]
async fn a_present_folder_is_matched_despite_a_trailing_slash() {
    // Idempotent across the same normalization: the service already holds the
    // canonical path, and a wanted path that differs only by a trailing slash is
    // left alone, not re-registered.
    let existing = vec![RegisteredFolder {
        id: "1".to_owned(),
        path: "/data/media/tv".to_owned(),
    }];
    let (states, recorded) = seed(
        FakeService::with(Mode::Normal, existing),
        &[folder("/data/media/tv/")],
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(recorded, 0);
}

/// The reason a shared root folder is refused, naming the given other \*arr, in
/// the exact words the driver builds so the assertions read against one source.
fn shared_root(path: &str, other: &str) -> String {
    format!(
        "{path} is also the root folder for {other}; two *arrs on one root folder would each manage the other's files"
    )
}

#[tokio::test]
async fn a_root_folder_another_arr_also_wants_is_refused_not_wired() {
    // Two *arrs on one root folder would each manage the other's files, so the
    // shared folder is refused — with the other *arr named — and nothing written.
    let contested = BTreeMap::from([(
        "/data/media/tv".to_owned(),
        vec!["radarr".to_owned(), "sonarr".to_owned()],
    )]);
    let (states, recorded) = seed_contested(
        FakeService::with(Mode::Normal, Vec::new()),
        &[folder("/data/media/tv")],
        &contested,
    )
    .await;
    assert_eq!(
        states,
        vec![State::Refused {
            reason: shared_root("/data/media/tv", "radarr"),
        }]
    );
    assert_eq!(recorded, 0, "a refused folder writes nothing");
}

#[tokio::test]
async fn a_contested_folder_is_refused_even_where_the_service_already_holds_it() {
    // The clash is the point: a folder already registered is still refused when
    // another *arr shares it, so the two are never left both managing one root.
    let existing = vec![RegisteredFolder {
        id: "1".to_owned(),
        path: "/data/media/tv".to_owned(),
    }];
    let contested = BTreeMap::from([(
        "/data/media/tv".to_owned(),
        vec!["radarr".to_owned(), "sonarr".to_owned()],
    )]);
    let (states, recorded) = seed_contested(
        FakeService::with(Mode::Normal, existing),
        &[folder("/data/media/tv")],
        &contested,
    )
    .await;
    assert_eq!(
        states,
        vec![State::Refused {
            reason: shared_root("/data/media/tv", "radarr"),
        }]
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn only_the_contested_folder_is_refused_the_rest_are_wired() {
    // Refusal is per folder: the shared one is refused, the *arr's own is wired.
    let contested = BTreeMap::from([(
        "/data/media/tv".to_owned(),
        vec!["radarr".to_owned(), "sonarr".to_owned()],
    )]);
    let (states, recorded) = seed_contested(
        FakeService::with(Mode::Normal, Vec::new()),
        &[folder("/data/media/tv"), folder("/data/media/movies")],
        &contested,
    )
    .await;
    assert_eq!(
        states,
        vec![
            State::Refused {
                reason: shared_root("/data/media/tv", "radarr"),
            },
            State::Wired,
        ]
    );
    assert_eq!(recorded, 1, "only the wired folder is journalled");
}

#[test]
fn a_root_folder_two_arrs_want_is_contested_naming_both_sorted() {
    let sonarr = [folder("/data/media/tv")];
    let radarr = [folder("/data/media/tv")];
    let contested = contested_roots([("sonarr", sonarr.as_slice()), ("radarr", radarr.as_slice())]);
    assert_eq!(
        contested.get("/data/media/tv").map(Vec::as_slice),
        Some(["radarr".to_owned(), "sonarr".to_owned()].as_slice()),
        "both *arrs are named, sorted so the reason reads the same either way",
    );
}

#[test]
fn a_root_folder_only_one_arr_wants_is_not_contested() {
    let sonarr = [folder("/data/media/tv")];
    let radarr = [folder("/data/media/movies")];
    let contested = contested_roots([("sonarr", sonarr.as_slice()), ("radarr", radarr.as_slice())]);
    assert!(
        contested.is_empty(),
        "no path is shared, so nothing is contested",
    );
}

#[test]
fn a_contested_path_is_recognised_across_a_trailing_slash() {
    // One *arr spells the path with a trailing slash, the other without; they are
    // the same folder, so the clash is not hidden.
    let sonarr = [folder("/data/media/tv/")];
    let radarr = [folder("/data/media/tv")];
    let contested = contested_roots([("sonarr", sonarr.as_slice()), ("radarr", radarr.as_slice())]);
    assert_eq!(contested.len(), 1);
    assert!(contested.contains_key("/data/media/tv"));
}

#[test]
fn one_arr_listing_a_path_twice_does_not_contest_itself() {
    // Distinct services, not repeats, make a contest: one *arr naming a path twice
    // is still one *arr.
    let sonarr = [folder("/data/media/tv"), folder("/data/media/tv")];
    let contested = contested_roots([("sonarr", sonarr.as_slice())]);
    assert!(
        contested.is_empty(),
        "one *arr cannot contest a folder with itself",
    );
}

#[tokio::test]
async fn a_root_folder_outside_the_data_root_is_refused_with_an_explanation() {
    // Every folder lemonfiber builds sits under /data, the mounted data root. One
    // that does not — reached beyond it — is refused, not created: the service
    // would file where its downloads are neither hardlinked to nor visible to the
    // rest of the stack. The refusal names the path and why, and writes nothing.
    let (states, recorded) = seed(
        FakeService::with(Mode::Normal, Vec::new()),
        &[folder("/config/media/tv")],
    )
    .await;
    let reason = match states.as_slice() {
        [State::Refused { reason }] => Some(reason.clone()),
        _ => None,
    };
    assert!(
        reason.is_some_and(|reason| {
            reason.contains("outside the data root") && reason.contains("/config/media/tv")
        }),
        "the refusal names the offending path and why: {states:?}"
    );
    assert_eq!(recorded, 0, "a refused folder writes nothing");
}

#[tokio::test]
async fn an_unavailable_service_skips_every_folder() {
    let (states, recorded) = seed(
        FakeService::with(Mode::Down, Vec::new()),
        &[folder("/data/media/tv"), folder("/data/media/movies")],
    )
    .await;
    assert!(
        states
            .iter()
            .all(|state| matches!(state, State::Skipped { .. })),
        "{states:?}"
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn a_service_that_refuses_the_listing_fails() {
    let (states, _) = seed(
        FakeService::with(Mode::RefusesList, Vec::new()),
        &[folder("/data/media/tv")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
}

#[tokio::test]
async fn a_service_on_an_unsupported_api_version_refuses_its_folders() {
    // The service answers, but not the API version this build speaks. Its folders
    // are refused, not skipped or failed: writing to it would be malformed, and a
    // re-run against the same service cannot lift it — the operator must align the
    // versions. Nothing is written.
    let (states, recorded) = seed(
        FakeService::with(Mode::Unsupported, Vec::new()),
        &[folder("/data/media/tv")],
    )
    .await;
    assert_eq!(
        states,
        vec![State::Refused {
            reason: "sonarr does not serve the API version this build speaks: there is no /api/v3"
                .to_owned(),
        }]
    );
    assert_eq!(recorded, 0, "nothing is written to an unsupported version");
}

#[tokio::test]
async fn a_rejected_registration_fails_with_the_services_own_words() {
    let (states, recorded) = seed(
        FakeService::with(Mode::RejectsRegister, Vec::new()),
        &[folder("/data/media/tv")],
    )
    .await;
    let detail = match states.as_slice() {
        [State::Failed { detail }] => Some(detail.clone()),
        _ => None,
    };
    assert!(
        detail.is_some_and(|words| words.contains("already used")),
        "the service's own words survive: {states:?}"
    );
    assert_eq!(recorded, 0, "a rejected write is not journalled");
}

#[tokio::test]
async fn a_write_that_does_not_appear_when_read_back_is_a_failure() {
    // The service accepted the registration but does not report the folder, so it
    // did not land — not done, and not recorded.
    let (states, recorded) = seed(
        FakeService::with(Mode::Swallows, Vec::new()),
        &[folder("/data/media/tv")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn a_service_that_stops_answering_after_the_write_is_skipped() {
    // The write went out but could not be confirmed, so it is left for a later
    // run to reconcile rather than declared wired.
    let (states, recorded) = seed(
        FakeService::with(Mode::DropsAfterRegister, Vec::new()),
        &[folder("/data/media/tv")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Skipped { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0, "an unconfirmed write is not recorded as done");
}
