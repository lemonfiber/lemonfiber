use std::sync::Arc;

use lemonfiber_contract::adapter::REQUEST_KEY_FILE;
use lemonfiber_fixtures::http::{Answer, Fake};

use super::super::tokening::tests::{
    accepting, fillers, minted, routes, scene, scene_beside, serving, stack, taking, CURATOR_KEY,
};
use super::{held, is_request_key, retired, rotate, SETTING};
use crate::credential::{Held, Reach, Settled, State};
use crate::test_support::curators::TELEVISION;
use crate::test_support::guards::Gate;
use crate::test_support::requests::{refusing, taken, REQUESTS};

/// What is written where the gate's keys are on the curator's adapter, in the stack at
/// `at`.
fn written(at: &std::path::Path) -> String {
    std::fs::read_to_string(key_at(at)).unwrap_or_default()
}

/// Where the gate's keys are on the curator's adapter, in the stack at `at`.
fn key_at(at: &std::path::Path) -> std::path::PathBuf {
    crate::plugin::key_file(at, TELEVISION.adapter).with_file_name(REQUEST_KEY_FILE)
}

/// A rotation's detail, where it was stopped unproven.
fn unproven(settled: &Settled) -> Option<&str> {
    match settled {
        Settled::Unproven { detail } => Some(detail),
        _ => None,
    }
}

/// The one line the curator has, in the stack at `at`.
async fn listed(ctx: &crate::app::Ctx, at: &std::path::Path) -> Option<Held> {
    held(ctx, &stack(true), &fillers(true, Some(at)))
        .await
        .into_iter()
        .next()
}

/// A line naming the route `route`.
fn naming(route: &str) -> Held {
    Held {
        name: "Request gate key on it".to_owned(),
        setting: format!("{SETTING}{route}"),
        consumers: Vec::new(),
        location: String::new(),
        origin: crate::credential::Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: State::Active,
        fingerprint: None,
        advisory: None,
    }
}

#[tokio::test]
async fn a_key_the_gate_presents_is_listed_active_without_its_value() {
    let (ctx, at, _) = scene(
        "requesting-listed",
        true,
        accepting(&[], &[]),
        taking("held", "linked"),
        true,
    );

    let line = listed(&ctx, &at).await;

    assert_eq!(
        line.as_ref().map(|one| (one.name.as_str(), &one.state)),
        Some(("Request gate key on Sonarr", &State::Active))
    );
    assert!(line.as_ref().is_some_and(is_request_key));
    assert_eq!(
        line.as_ref().map(|one| one.setting.as_str()),
        Some("request-gate/request.key#tv")
    );
    let shown = format!("{line:?}");
    assert!(!shown.contains(CURATOR_KEY), "{shown}");
    assert_eq!(
        line.and_then(|one| one.fingerprint),
        Some(crate::credential::fingerprint(CURATOR_KEY))
    );
}

#[tokio::test]
async fn a_key_the_gate_does_not_present_is_absent_and_says_what_writes_one() {
    let (ctx, at, _) = scene(
        "requesting-absent",
        true,
        accepting(&[], &[]),
        taking("held", "linked"),
        true,
    );
    TELEVISION.gate_key(&at, "another");

    let line = listed(&ctx, &at).await;

    assert_eq!(line.as_ref().map(|one| &one.state), Some(&State::Absent));
    assert!(line
        .and_then(|one| one.advisory)
        .is_some_and(|said| said.contains("Run lemonfiber seed")));
}

#[tokio::test]
async fn a_key_kept_where_a_link_stands_is_stale_and_says_so() {
    let (ctx, at, _) = scene(
        "requesting-stale",
        true,
        accepting(&[], &[]),
        taking("held", "linked"),
        true,
    );
    let _ = std::fs::remove_file(key_at(&at));
    let _ = std::os::unix::fs::symlink(at.join(".env"), key_at(&at));

    let line = listed(&ctx, &at).await;

    assert_eq!(line.as_ref().map(|one| &one.state), Some(&State::Stale));
}

#[tokio::test]
async fn a_stack_without_the_gate_or_a_curator_it_routes_to_lists_nothing() {
    let (ctx, at, _) = scene(
        "requesting-none",
        true,
        accepting(&[], &[]),
        taking("held", "linked"),
        true,
    );

    assert!(held(&ctx, &stack(false), &fillers(false, Some(&at)))
        .await
        .is_empty());
    assert!(held(
        &ctx,
        &stack(true),
        &crate::wiring::Fillers::holding(Vec::new())
    )
    .await
    .is_empty());
}

#[tokio::test]
async fn a_gate_that_cannot_be_asked_leaves_the_line_absent() {
    let (ctx, at, _) = scene(
        "requesting-unasked",
        true,
        accepting(&[], &[]),
        taking("held", "linked"),
        true,
    );
    let without_a_gate_to_ask = crate::test_support::a_context()
        .settings(ctx.settings.clone())
        .build();

    let line = listed(&without_a_gate_to_ask, &at).await;

    assert_eq!(line.map(|one| one.state), Some(State::Absent));
}

/// A key on a curator's adapter is never printed, however confirmed the asking.
#[tokio::test]
async fn a_request_key_is_not_printed_however_confirmed() {
    let (ctx, at, _gate) = scene(
        "requesting-unprinted",
        true,
        accepting(&["token"], &["linked"]),
        taking("held", "linked"),
        true,
    );

    let revealed = crate::app::credentials::revealing::reveal(
        &ctx,
        &naming("tv"),
        true,
        &fillers(true, Some(&at)),
    )
    .await;

    assert_eq!(revealed.value, None);
    assert_eq!(revealed.warning, super::UNPRINTED);
}

/// A rotation asked for by the line reaches the rotation of the gate's key on the adapter.
#[tokio::test]
async fn a_rotation_of_the_line_replaces_the_key_on_the_adapter() {
    let (ctx, at, _gate) = scene(
        "requesting-dispatched",
        true,
        accepting(&["token"], &["linked"]),
        taking("held", "linked"),
        true,
    );

    let rotation = crate::app::credentials::rotating::rotate(
        &ctx,
        &naming("tv"),
        &stack(true),
        &fillers(true, Some(&at)),
    )
    .await;

    assert!(
        matches!(rotation.settled, Settled::Replaced { .. }),
        "{rotation:?}"
    );
}

#[tokio::test]
async fn a_rotation_lands_once_the_request_service_proves_the_new_key_and_retires_the_old() {
    let (ctx, at, gate) = scene(
        "requesting-landed",
        true,
        accepting(&["token"], &["linked"]),
        taking("held", "linked"),
        true,
    );

    let rotation = rotate(&ctx, &naming("tv"), &fillers(true, Some(&at))).await;

    assert!(
        matches!(rotation.settled, Settled::Replaced { .. }),
        "{rotation:?}"
    );
    assert_eq!(written(&at), format!("{}\n", minted()));
    let mut expected = routes(accepting(&["token"], &["linked"]));
    if let Some(curator) = expected.first_mut() {
        curator.credential = minted();
    }
    assert_eq!(gate.routes(), expected);
    assert_eq!(
        rotation.consumers.first().map(|one| one.reach.clone()),
        Some(Reach::Updated)
    );
}

#[tokio::test]
async fn a_rehearsal_writes_nothing_and_says_what_a_run_would_do() {
    let (mut ctx, at, gate) = scene(
        "requesting-rehearsed",
        true,
        accepting(&[], &[]),
        taking("held", "linked"),
        true,
    );
    ctx.dry_run = true;
    let before = gate.routes();

    let rotation = rotate(&ctx, &naming("tv"), &fillers(true, Some(&at))).await;

    assert!(
        matches!(rotation.settled, Settled::Rehearsed { .. }),
        "{rotation:?}"
    );
    assert_eq!(written(&at), format!("{CURATOR_KEY}\n"));
    assert_eq!(gate.routes(), before);
}

#[tokio::test]
async fn a_new_key_the_request_service_cannot_prove_is_taken_out_and_the_old_routes_handed_back() {
    let http = serving("held", "linked", taken(), vec![refusing()], taken());
    let (ctx, at, gate) = scene("requesting-unproven", true, accepting(&[], &[]), http, true);
    let before = gate.routes();

    let rotation = rotate(&ctx, &naming("tv"), &fillers(true, Some(&at))).await;

    assert!(
        unproven(&rotation.settled).is_some_and(|said| said.contains("could not reach Sonarr")),
        "{rotation:?}"
    );
    assert_eq!(written(&at), format!("{CURATOR_KEY}\n"));
    assert_eq!(gate.routes(), before);
}

#[tokio::test]
async fn a_new_key_the_gate_will_not_take_is_taken_out() {
    let told = Gate::admitting(routes(accepting(&[], &[]))).refusing(&["admit"]);
    let (ctx, at, _) = scene_beside(
        "requesting-unadmitted",
        told,
        taking("held", "linked"),
        true,
    );

    let rotation = rotate(&ctx, &naming("tv"), &fillers(true, Some(&at))).await;

    assert!(
        unproven(&rotation.settled).is_some_and(|said| said.contains("could not be proven")),
        "{rotation:?}"
    );
    assert_eq!(written(&at), format!("{CURATOR_KEY}\n"));
}

#[tokio::test]
async fn nothing_is_replaced_where_there_is_nothing_to_replace_or_it_cannot_be_written() {
    let scened = |name: &'static str, random: bool, http: Arc<Fake>| {
        scene(name, true, accepting(&[], &[]), http, random)
    };
    let said = |rotation: &crate::credential::Rotation, words: &str| {
        assert!(
            unproven(&rotation.settled).is_some_and(|said| said.contains(words)),
            "{words}: {rotation:?}"
        );
    };

    let (ctx, at, _) = scened("requesting-unrouted", true, taking("held", "linked"));
    let filled = fillers(true, Some(&at));
    said(
        &rotate(&ctx, &naming("movies"), &filled).await,
        "not presented by the request gate yet",
    );

    let unasked = crate::test_support::a_context()
        .settings(ctx.settings.clone())
        .build();
    said(
        &rotate(&unasked, &naming("tv"), &filled).await,
        "not presented by the request gate yet",
    );

    TELEVISION.gate_key(&at, "another");
    said(
        &rotate(&ctx, &naming("tv"), &filled).await,
        "not presented by the request gate yet",
    );

    let _ = std::fs::remove_file(key_at(&at));
    let _ = std::os::unix::fs::symlink(at.join(".env"), key_at(&at));
    said(
        &rotate(&ctx, &naming("tv"), &filled).await,
        "not a plain file of its own",
    );

    let (ctx, at, _) = scened("requesting-unminted", false, taking("held", "linked"));
    said(
        &rotate(&ctx, &naming("tv"), &fillers(true, Some(&at))).await,
        crate::secret::NO_RANDOMNESS_FOR_TOKEN,
    );
    assert_eq!(written(&at), format!("{CURATOR_KEY}\n"));

    let (ctx, at, _) = scened("requesting-unkeyed", true, taking("held", "linked"));
    let _ = std::fs::remove_file(crate::plugin::key_file(&at, REQUESTS.adapter));
    said(
        &rotate(&ctx, &naming("tv"), &fillers(true, Some(&at))).await,
        "not presented by the request gate yet",
    );
    assert_eq!(written(&at), format!("{CURATOR_KEY}\n"));
}

#[tokio::test]
async fn the_gates_media_server_key_or_another_curators_unread_stops_a_rotation_first() {
    let (ctx, at, _) = scene(
        "requesting-unread-media",
        true,
        accepting(&[], &[]),
        serving_no_key(),
        true,
    );
    let rotation = rotate(&ctx, &naming("tv"), &fillers(true, Some(&at))).await;
    assert!(
        unproven(&rotation.settled).is_some_and(|said| said.contains("every route at once")),
        "{rotation:?}"
    );

    let mut kept = routes(accepting(&[], &[]));
    kept.push(crate::ports::service::Gated {
        target: crate::ports::service::Target::Curator {
            media_type: "movies".to_owned(),
        },
        path: "movies".to_owned(),
        address: "http://radarr-adapter:8080".to_owned(),
        majors: Vec::new(),
        credential: "unknown".to_owned(),
        tokens: Vec::new(),
    });
    let (ctx, at, _) = scene_beside(
        "requesting-unread-curator",
        Gate::admitting(kept),
        taking("held", "linked"),
        true,
    );
    let rotation = rotate(&ctx, &naming("tv"), &fillers(true, Some(&at))).await;
    assert!(
        unproven(&rotation.settled).is_some_and(|said| said.contains("holds no copy")),
        "{rotation:?}"
    );
    assert_eq!(written(&at), format!("{CURATOR_KEY}\n"));
}

/// A media server filing no key under the gate's name.
fn serving_no_key() -> Arc<Fake> {
    Fake::by_path_in_turn(vec![(
        crate::test_support::media::serve("filed_as"),
        vec![Answer::reply(200, "[]".to_owned())],
    )])
}

#[test]
fn an_old_key_that_cannot_be_taken_out_is_said() {
    assert_eq!(retired(Ok(())), Reach::Updated);
    assert_eq!(
        retired(Err("the disk".to_owned())),
        Reach::Failed {
            detail: "the disk".to_owned()
        }
    );
}
