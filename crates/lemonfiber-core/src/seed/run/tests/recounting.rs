//! Each curator and media server asked to tell the core what happened, with a key minted
//! for it alone.

use super::*;
use crate::test_support::{contracted, first_party};

/// The plugin whose adapter speaks `media.serve`.
const PLUGIN: &str = "serving";

/// The media server it speaks for.
const SERVICE: &str = "served-part";

/// Where the media server's adapter answers `events`.
const EVENTS: &str = "/lemonfiber/media.serve/v1/events";

/// Where the core is told what happened, in these cases.
const TOLD_AT: &str = "https://core.home:8443/api/notify/events";

/// The certificate digest the core presents there, in these cases.
const PINNED: &str = "ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12";

/// Where the core is told what happened, and what it pins, in these cases.
fn told_at() -> crate::notify::events::Told {
    crate::notify::events::Told {
        address: TOLD_AT.to_owned(),
        pin: PINNED.to_owned(),
    }
}

/// The stack of a media server, trusted as first-party.
fn stack(project: &std::path::Path) -> crate::wiring::Fillers {
    fillers_trusting(
        Vec::new(),
        &[contracted(PLUGIN, SERVICE, "media.serve")],
        project,
        &first_party(PLUGIN),
    )
}

/// The states the wiring came to.
fn states(wirings: &[crate::seed::Wiring]) -> Vec<crate::seed::State> {
    wirings.iter().map(|one| one.state.clone()).collect()
}

#[tokio::test]
async fn with_no_address_known_no_service_is_asked_and_each_says_why() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("recounting-nowhere");
    let http = Fake::by_path(vec![(EVENTS, Answer::reply(204, ""))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());

    let wirings = super::super::recounting::seed_events(
        &ctx,
        &stack(&project),
        Err("no address here".to_owned()),
    )
    .await;

    assert!(
        matches!(states(&wirings).as_slice(), [crate::seed::State::Skipped { reason }] if reason.contains("no address")),
        "{wirings:?}"
    );
    assert!(!http.asked_for(EVENTS));
}

#[tokio::test]
async fn a_media_server_is_handed_the_address_and_a_key_minted_for_it_alone() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("recounting-asked");
    let http = Fake::by_path(vec![(EVENTS, Answer::reply(204, ""))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());

    let wirings =
        super::super::recounting::seed_events(&ctx, &stack(&project), Ok(told_at())).await;

    assert_eq!(states(&wirings), [crate::seed::State::Wired], "{wirings:?}");
    let body = http
        .requests()
        .into_iter()
        .find(|one| one.url.ends_with(EVENTS))
        .and_then(|one| one.body)
        .unwrap_or_default();
    assert!(body.contains(TOLD_AT), "{body}");
    assert!(body.contains(PINNED), "{body}");
    let key = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|asked| {
            asked
                .get("key")
                .and_then(|key| key.as_str())
                .map(str::to_owned)
        })
        .unwrap_or_default();
    let written = stack(&project)
        .service(SERVICE)
        .and_then(|one| one.confined_to.clone())
        .and_then(|dir| {
            std::fs::read_to_string(dir.join(lemonfiber_contract::adapter::EVENTS_KEY_FILE)).ok()
        });
    assert_eq!(written.as_deref().map(str::trim), Some(key.as_str()));
    let told = crate::notify::events::Event {
        event: crate::notify::events::Happened::Added,
        reference: None,
        name: "Dune".to_owned(),
        reason: None,
        cleared: false,
    };
    assert_eq!(
        crate::notify::events::intake(&ctx, &key, &told)
            .ok()
            .as_deref(),
        Some(SERVICE)
    );
}

#[tokio::test]
async fn a_rehearsal_asks_nothing_and_says_where_it_would_point() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("recounting-rehearsed");
    let http = Fake::by_path(vec![(EVENTS, Answer::reply(204, ""))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone()).rehearsing();

    let wirings =
        super::super::recounting::seed_events(&ctx, &stack(&project), Ok(told_at())).await;

    assert!(
        matches!(states(&wirings).as_slice(), [crate::seed::State::WouldWire { ours: Some(at), .. }] if at == TOLD_AT),
        "{wirings:?}"
    );
    assert!(!http.asked_for(EVENTS));
}

#[tokio::test]
async fn a_media_server_on_an_earlier_minor_is_left_as_it_is() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("recounting-unsupported");
    let http = Fake::by_path(vec![(
        EVENTS,
        Answer::served(
            500,
            crate::ports::http::PROBLEM,
            r#"{"type":"unsupported","detail":"events came later"}"#,
        ),
    )]);
    let ctx = contracted_ctx(&project, SERVICE, true, http);

    let wirings = told(&ctx, &stack(&project)).await;

    assert!(
        matches!(
            states(&wirings).as_slice(),
            [crate::seed::State::Unmatched { reason }] if reason.contains("events came later")
        ),
        "{wirings:?}"
    );
}

/// What seeding the events comes to for `fillers`, the core told at [`TOLD_AT`].
async fn told(ctx: &Ctx, fillers: &crate::wiring::Fillers) -> Vec<crate::seed::Wiring> {
    super::super::recounting::seed_events(ctx, fillers, Ok(told_at())).await
}

/// The one wiring's failure, where it failed.
fn failed(wirings: &[crate::seed::Wiring]) -> Option<String> {
    match states(wirings).as_slice() {
        [crate::seed::State::Failed { detail }] => Some(detail.clone()),
        _ => None,
    }
}

#[tokio::test]
async fn a_curator_is_handed_the_address_over_its_own_contract() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("recounting-curator");
    let asked_at = "/lemonfiber/library.curate/v1/events";
    let http = Fake::by_path(vec![(asked_at, Answer::reply(204, ""))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let curating = fillers_trusting(
        Vec::new(),
        &[contracted(PLUGIN, SERVICE, "library.curate")],
        &project,
        &first_party(PLUGIN),
    );

    let wirings = told(&ctx, &curating).await;

    assert_eq!(states(&wirings), [crate::seed::State::Wired], "{wirings:?}");
    assert!(http.asked_for(asked_at));
}

#[tokio::test]
async fn a_key_that_cannot_be_kept_is_never_handed_over() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("recounting-unkept");
    let http = Fake::by_path(vec![(EVENTS, Answer::reply(204, ""))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let record = crate::app::targets::beside_env(&ctx, crate::config::paths::NOTIFY_EVENTS)
        .unwrap_or_default();
    let blocked = crate::test_support::blocking(&record);

    let wirings = told(&ctx, &stack(&project)).await;

    assert!(failed(&wirings).is_some(), "{wirings:?}");
    assert!(!http.asked_for(EVENTS));
    assert!(std::fs::remove_dir_all(blocked).is_ok());
}

#[tokio::test]
async fn a_key_that_cannot_be_written_beside_the_service_is_never_handed_over() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("recounting-unwritten");
    let http = Fake::by_path(vec![(EVENTS, Answer::reply(204, ""))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let fillers = stack(&project);
    let within = fillers
        .service(SERVICE)
        .and_then(|one| one.confined_to.clone())
        .unwrap_or_default();
    let key_file = within.join(lemonfiber_contract::adapter::EVENTS_KEY_FILE);
    assert!(std::fs::create_dir_all(key_file.join("held")).is_ok());

    let wirings = told(&ctx, &fillers).await;

    assert!(
        failed(&wirings).is_some_and(|detail| detail.contains("could not be written")),
        "{wirings:?}"
    );
    assert!(!http.asked_for(EVENTS));
}

#[tokio::test]
async fn a_service_this_machine_cannot_reach_over_its_contract_is_not_asked() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("recounting-unreached");
    let http = Fake::by_path(vec![(EVENTS, Answer::reply(204, ""))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let unplaced = crate::test_support::stack_fillers(
        Vec::new(),
        &[contracted(PLUGIN, SERVICE, "media.serve")],
        None,
        &first_party(PLUGIN),
    );

    let wirings = told(&ctx, &unplaced).await;

    assert!(
        matches!(states(&wirings).as_slice(), [crate::seed::State::Skipped { reason }] if reason.contains("could not be asked")),
        "{wirings:?}"
    );
    assert!(!http.asked_for(EVENTS));
}

#[test]
fn a_service_with_no_configuration_here_is_written_no_key() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("recounting-unconfined");
    let ctx = contracted_ctx(&project, SERVICE, true, Fake::silent());
    let homeless = stack(&project).service(SERVICE).cloned().map(|mut one| {
        one.confined_to = None;
        one
    });

    let written = homeless
        .map(|one| super::super::recounting::written(&ctx, &one, "the-key"))
        .and_then(Result::err);

    assert!(
        written
            .as_deref()
            .is_some_and(|why| why.contains("no configuration on this machine")),
        "{written:?}"
    );
}

#[tokio::test]
async fn a_service_refusing_the_ask_is_reported_failed() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("recounting-refused");
    let http = Fake::by_path(vec![(EVENTS, Answer::reply(401, ""))]);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());

    let wirings = told(&ctx, &stack(&project)).await;

    assert!(failed(&wirings).is_some(), "{wirings:?}");
    assert!(http.asked_for(EVENTS));
}
