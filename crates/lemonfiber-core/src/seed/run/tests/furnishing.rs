//! A library for each media type the stack files, on every media server.

use std::collections::BTreeSet;

use super::*;
use crate::baseline::Baseline;
use crate::test_support::{contracted, first_party};

/// The plugin whose adapter speaks `media.serve`.
const PLUGIN: &str = "serving";

/// The media server it speaks for.
const SERVICE: &str = "served-part";

/// Where the adapter answers `furnish`.
const FURNISH: &str = "/lemonfiber/media.serve/v1/furnish";

/// The media server's plugin, speaking `media.serve` and each of `also`.
fn server(also: &[&str]) -> crate::plugin::Installed {
    let mut installed = contracted(PLUGIN, SERVICE, "media.serve");
    for placed in &mut installed.services {
        placed
            .speaks
            .extend(also.iter().map(|one| format!("{one}@1")));
    }
    installed
}

/// A curator's plugin filing each of `media` over `library.curate`.
fn curating(media: &[&str]) -> crate::plugin::Installed {
    let mut installed = contracted("curating", "shows", "library.curate");
    for placed in &mut installed.services {
        placed.media_types = media.iter().map(|one| (*one).to_owned()).collect();
    }
    installed
}

/// The stack of `installed`, trusting the media server's plugin as first-party.
fn stack(
    project: &std::path::Path,
    installed: &[crate::plugin::Installed],
) -> crate::wiring::Fillers {
    fillers_trusting(Vec::new(), installed, project, &first_party(PLUGIN))
}

/// The adapter answering `furnish` with `body`.
fn answering(body: &str) -> Arc<Fake> {
    Fake::by_path(vec![(FURNISH, Answer::reply(200, body))])
}

/// What the media server is given on a seed of `fillers`, with `baseline` and `kept_off`.
async fn furnished(
    ctx: &Ctx,
    fillers: &crate::wiring::Fillers,
    baseline: &Baseline,
    kept_off: &BTreeSet<String>,
) -> Vec<crate::seed::Wiring> {
    super::super::furnishing::seed_furnishing(ctx, fillers, baseline, kept_off).await
}

/// The bodies the adapter was asked `furnish` with.
fn asked(http: &Fake) -> Vec<String> {
    http.requests()
        .into_iter()
        .filter(|one| one.url.ends_with(FURNISH))
        .filter_map(|one| one.body)
        .collect()
}

#[tokio::test]
async fn a_media_server_is_handed_each_media_type_the_stack_files_with_its_folder() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("furnishing-handed");
    let http =
        answering(r#"[{"media_type":"movies","held":true},{"media_type":"tv","held":true}]"#);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let fillers = stack(&project, &[server(&[]), curating(&["tv", "movies"])]);

    let wirings = furnished(&ctx, &fillers, &Baseline::new(), &BTreeSet::new()).await;

    assert!(
        matches!(wirings.as_slice(), [one] if one.state == crate::seed::State::Wired),
        "{wirings:?}"
    );
    let bodies = asked(&http);
    assert_eq!(bodies.len(), 1, "{bodies:?}");
    for media in ["tv", "movies"] {
        let folder = super::super::baseline::media_folder(media);
        assert!(
            bodies
                .iter()
                .any(|body| body.contains(&format!("\"{media}\"")) && body.contains(&folder)),
            "{media} at {folder} missing from {bodies:?}"
        );
    }
}

#[tokio::test]
async fn a_media_server_that_cannot_be_given_a_library_is_left_unmatched_saying_why() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("furnishing-unsupported");
    let http = Fake::by_path(vec![(
        FURNISH,
        Answer::served(
            500,
            crate::ports::http::PROBLEM,
            r#"{"type":"unsupported","detail":"its libraries are kept by hand"}"#,
        ),
    )]);
    let ctx = contracted_ctx(&project, SERVICE, true, http);
    let fillers = stack(&project, &[server(&[]), curating(&["tv"])]);

    let wirings = furnished(&ctx, &fillers, &Baseline::new(), &BTreeSet::new()).await;

    assert!(
        matches!(wirings.as_slice(), [one] if matches!(&one.state,
            crate::seed::State::Unmatched { reason }
                if reason.contains("its libraries are kept by hand") && reason.contains(SERVICE))),
        "{wirings:?}"
    );
}

#[tokio::test]
async fn a_media_server_refusing_furnish_is_not_reported_wired() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("furnishing-refused");
    let http = Fake::by_path(vec![(
        FURNISH,
        Answer::served(
            502,
            crate::ports::http::PROBLEM,
            r#"{"type":"refused","detail":"the server said no"}"#,
        ),
    )]);
    let ctx = contracted_ctx(&project, SERVICE, true, http);
    let fillers = stack(&project, &[server(&[]), curating(&["tv"])]);

    let wirings = furnished(&ctx, &fillers, &Baseline::new(), &BTreeSet::new()).await;

    assert!(
        matches!(wirings.as_slice(), [one] if !matches!(one.state,
            crate::seed::State::Wired | crate::seed::State::Unmatched { .. })),
        "{wirings:?}"
    );
}

#[tokio::test]
async fn a_rehearsal_names_the_media_types_and_asks_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("furnishing-rehearsed");
    let http = answering("[]");
    let mut ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    ctx.dry_run = true;
    let fillers = stack(&project, &[server(&[]), curating(&["tv", "movies"])]);

    let wirings = furnished(&ctx, &fillers, &Baseline::new(), &BTreeSet::new()).await;

    assert!(
        matches!(wirings.as_slice(), [one] if matches!(&one.state,
            crate::seed::State::WouldWire { yours: None, ours: Some(ours) } if ours == "movies, tv")),
        "{wirings:?}"
    );
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[tokio::test]
async fn a_media_server_claimed_over_owner_claim_is_asked_only_once_it_is_ours() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("furnishing-claimed");
    let http = answering(r#"[{"media_type":"tv","held":true}]"#);
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let fillers = stack(&project, &[server(&["owner.claim"]), curating(&["tv"])]);
    let mut ours = Baseline::new();
    ours.record(
        SERVICE,
        super::super::owning::FIELD,
        "ours",
        "2026-10-31T00:00:00Z",
    );

    let unheld = furnished(&ctx, &fillers, &Baseline::new(), &BTreeSet::new()).await;
    let held = furnished(&ctx, &fillers, &ours, &BTreeSet::new()).await;

    assert!(unheld.is_empty(), "{unheld:?}");
    assert!(
        matches!(held.as_slice(), [one] if one.state == crate::seed::State::Wired),
        "{held:?}"
    );
    assert_eq!(asked(&http).len(), 1);
}

#[tokio::test]
async fn a_media_server_kept_off_is_asked_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("furnishing-kept-off");
    let http = answering("[]");
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let fillers = stack(&project, &[server(&[]), curating(&["tv"])]);

    let wirings = furnished(
        &ctx,
        &fillers,
        &Baseline::new(),
        &BTreeSet::from([SERVICE.to_owned()]),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[tokio::test]
async fn a_stack_filing_nothing_asks_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("furnishing-nothing-filed");
    let http = answering("[]");
    let ctx = contracted_ctx(&project, SERVICE, true, http.clone());
    let fillers = stack(&project, &[server(&[])]);

    let wirings = furnished(&ctx, &fillers, &Baseline::new(), &BTreeSet::new()).await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[tokio::test]
async fn a_media_server_whose_adapter_cannot_be_reached_is_skipped_and_asked_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("furnishing-unreached");
    let http = answering("[]");
    let ctx = seed_ctx(false, None, None).with_http(http.clone());
    let fillers = stack(&project, &[server(&[]), curating(&["tv"])]);

    let wirings = furnished(&ctx, &fillers, &Baseline::new(), &BTreeSet::new()).await;

    assert!(
        matches!(wirings.as_slice(), [one] if matches!(&one.state,
            crate::seed::State::Skipped { reason } if reason.contains(SERVICE))),
        "{wirings:?}"
    );
    assert!(asked(&http).is_empty(), "{:?}", http.requests());
}
