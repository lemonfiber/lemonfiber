use super::{named, narrowed, still_transferring, update, Asked, Report};
use crate::app::engine::Interrupted;
use crate::app::{Ctx, Waiting};
use crate::dashboard::Protocol;
use crate::test_support::{a_context, nowhere};
use crate::update::State as Standing;
use lemonfiber_fixtures::pulled::Pulled;

/// What was asked, with nothing narrowed and nothing agreed to.
fn asking() -> Asked {
    Asked {
        service: None,
        confirm: false,
        wait: Waiting::Never,
    }
}

/// A run against an engine that has pulled nothing.
///
/// Named rather than left to the default, which is this machine's own daemon: a
/// test that read a real engine would answer differently on every machine it ran
/// on, and would answer nothing at all where no daemon is installed.
fn nothing_pulled() -> Ctx {
    a_context().build().with_images(Pulled::holding(Vec::new()))
}

/// The manifest the shipped stack declares.
fn manifest() -> Option<lemonfiber_manifest::Manifest> {
    let ctx = a_context().build();
    let today = ctx.today();
    ctx.stack.checked_manifest(today).ok()
}

#[tokio::test]
async fn a_stack_whose_engine_answers_nothing_is_already_on_its_pins() {
    let ctx = nothing_pulled();
    let read = update(&ctx, asking()).await;
    let state = read.map(|report| (report.state, report.changes.len(), report.confirmed));
    assert_eq!(state.ok(), Some((Standing::Current, 0, false)));
}

#[tokio::test]
async fn a_stack_that_cannot_be_read_is_refused_rather_than_reported_as_current() {
    let ctx = a_context()
        .over(nowhere())
        .build()
        .with_images(Pulled::holding(Vec::new()));
    let read = update(&ctx, asking()).await;
    assert!(
        read.is_err(),
        "a stack nothing could read said what it holds"
    );
}

#[tokio::test]
async fn an_engine_that_will_not_say_what_it_pulled_stops_the_reading() {
    let ctx = a_context()
        .build()
        .with_images(Pulled::unreachable("no daemon here"));
    let read = update(&ctx, asking()).await;
    let code = read.err().map(|problem| problem.code.to_string());
    assert_eq!(code.as_deref(), Some("UPDATE-1"));
}

#[tokio::test]
async fn a_service_the_stack_does_not_declare_is_refused_by_name() {
    let ctx = nothing_pulled();
    let read = update(
        &ctx,
        Asked {
            service: Some("not-a-service".to_owned()),
            ..asking()
        },
    )
    .await;
    let code = read.err().map(|problem| problem.code.to_string());
    assert_eq!(code.as_deref(), Some("UPDATE-2"));
}

#[test]
fn narrowing_to_one_service_leaves_the_rest_of_the_stack_alone() {
    let manifest = manifest();
    let only = manifest
        .as_ref()
        .and_then(|manifest| narrowed(manifest, Some("sonarr")).ok())
        .unwrap_or_default();
    let names: Vec<String> = only.into_iter().map(|pin| pin.service).collect();
    assert_eq!(names, vec!["sonarr".to_owned()]);
}

#[test]
fn narrowing_to_nothing_takes_every_service_the_stack_declares() {
    let manifest = manifest();
    let every = manifest
        .as_ref()
        .and_then(|manifest| narrowed(manifest, None).ok())
        .unwrap_or_default();
    let count = every.len();
    assert!(count > 10, "{count} services");
}

#[test]
fn what_is_in_flight_is_named_with_how_far_along_it_is() {
    let active = [Interrupted {
        protocol: Protocol::Torrent,
        name: "Ubuntu.iso".to_owned(),
        progress: 94,
    }];
    assert_eq!(named(&active), vec!["Ubuntu.iso (94%)".to_owned()]);
}

#[test]
fn a_run_that_would_interrupt_a_transfer_says_how_to_let_it_finish() {
    let problem = still_transferring(&["Ubuntu.iso (94%)".to_owned()]);
    let remedy = problem.remedies.first().and_then(|one| one.detail.clone());
    assert_eq!(problem.code.to_string(), "UPDATE-3");
    assert_eq!(
        remedy.as_deref(),
        Some("lemonfiber update --confirm --wait")
    );
}

#[test]
fn a_proposal_carries_nothing_a_run_would_have_left_behind() {
    let report = Report::proposed(Vec::new(), Vec::new(), false);
    assert_eq!(report.clone(), report);
    assert_eq!(report.backup, None);
    assert_eq!(report.halted, None);
    assert!(report.applied.is_empty());
}
