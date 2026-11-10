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

    let wirings = super::super::recounting::seed_events(&ctx, &stack(&project), None).await;

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
        super::super::recounting::seed_events(&ctx, &stack(&project), Some(TOLD_AT.to_owned()))
            .await;

    assert_eq!(states(&wirings), [crate::seed::State::Wired], "{wirings:?}");
    let body = http
        .requests()
        .into_iter()
        .find(|one| one.url.ends_with(EVENTS))
        .and_then(|one| one.body)
        .unwrap_or_default();
    assert!(body.contains(TOLD_AT), "{body}");
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
        super::super::recounting::seed_events(&ctx, &stack(&project), Some(TOLD_AT.to_owned()))
            .await;

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
        Answer::reply(
            501,
            r#"{"code":"SERVICE-4","summary":"not this minor","detail":"events came later"}"#,
        ),
    )]);
    let ctx = contracted_ctx(&project, SERVICE, true, http);

    let wirings =
        super::super::recounting::seed_events(&ctx, &stack(&project), Some(TOLD_AT.to_owned()))
            .await;

    assert!(
        matches!(
            states(&wirings).as_slice(),
            [crate::seed::State::Unmatched { .. } | crate::seed::State::Failed { .. }]
        ),
        "{wirings:?}"
    );
}
