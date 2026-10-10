//! Standing in place of an existing setup.
//!
//! Its yes is the offer its reading named. Each run here reads first and answers what
//! it read, the way a surface does across two requests, because the answer is only a
//! yes to the reading it was given for.

use super::common::stack::project;
use super::{driven, over, replacing, replacing_as, somebody_elses};
use lemonfiber_core::app::{Ctx, MigrateAction};
use lemonfiber_core::error::codes::migrate::OFFER_MOVED;
use lemonfiber_core::migration::mode::Mode;
use lemonfiber_core::model::ReplaceReport;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::Runner;
use lemonfiber_core::reconfigure::Stance;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{refused, spoke, Recording, Reporting};
use std::sync::Arc;

/// Somebody else's curator, standing here, with every command run recorded.
fn theirs(runner: &Arc<Recording>) -> Ctx {
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.15",
        400,
        &["media"],
    )]);
    driven(
        somebody_elses(),
        images,
        Source::External(project()),
        Arc::clone(runner) as Arc<dyn Runner>,
    )
}

/// What reading the offer and then answering it came to.
async fn read_then_answered(ctx: &Ctx) -> Option<ReplaceReport> {
    let read = replacing(ctx, None).await.ok().flatten()?;
    replacing(ctx, Some(&read.agreement)).await.ok().flatten()
}

#[tokio::test]
async fn asked_without_an_offer_it_names_what_would_stop_and_the_offer_and_stops_nothing() {
    let watching = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = theirs(&watching);

    let found = replacing(&ctx, None).await.ok().flatten();
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Pending),
        "{found:?}"
    );
    let offer = found
        .as_ref()
        .map(|read| read.agreement.clone())
        .unwrap_or_default();
    assert_eq!(offer.len(), 8, "the offer names itself: {found:?}");
    let named = found.map(|read| read.would_stop).unwrap_or_default();
    assert_eq!(named, vec!["sonarr".to_owned()], "what would stop");
    assert!(
        watching.seen().is_empty(),
        "a reading ran {:?}",
        watching.seen()
    );
}

#[tokio::test]
async fn the_same_machine_read_twice_names_the_same_offer() {
    let ctx = theirs(&Arc::new(Recording::answering(Ok(spoke("")))));
    let once = replacing(&ctx, None)
        .await
        .ok()
        .flatten()
        .map(|read| read.agreement);
    let again = replacing(&ctx, None)
        .await
        .ok()
        .flatten()
        .map(|read| read.agreement);
    assert!(once.is_some(), "it was read");
    assert_eq!(once, again);
}

#[tokio::test]
async fn answering_the_offer_stops_what_it_named_and_deletes_none_of_it() {
    let watching = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = theirs(&watching);

    let found = read_then_answered(&ctx).await;
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Applied),
        "{found:?}"
    );

    // Everything it ran, not one word at a time: an invocation that removed rather than
    // stopped is exactly what a narrower question would miss.
    let ran = watching.seen();
    let stopping: Vec<&Vec<String>> = ran
        .iter()
        .filter(|argv| argv.contains(&"stop".to_owned()))
        .collect();
    assert_eq!(stopping.len(), 1, "one stop per running container: {ran:?}");
    let destructive = ran.iter().any(|argv| {
        argv.iter()
            .any(|word| word == "rm" || word == "down" || word == "prune")
    });
    assert!(!destructive, "nothing of theirs was removed: {ran:?}");
}

#[tokio::test]
async fn an_offer_that_has_moved_is_refused_by_name_and_stops_nothing() {
    let watching = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = theirs(&watching);

    let refused = replacing(&ctx, Some("00000000")).await.err();
    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(OFFER_MOVED)
    );
    assert_eq!(
        refused.as_ref().map(|problem| problem.status()),
        Some(400),
        "answered as an answer to correct, never as a failure of the machine"
    );
    // What moved is named: what it would stop now, beside both names.
    let said = refused
        .map(|problem| format!("{} {}", problem.meaning, problem.detail.unwrap_or_default()))
        .unwrap_or_default();
    assert!(said.contains("00000000"), "{said}");
    assert!(said.contains("sonarr"), "{said}");
    assert!(
        watching.seen().is_empty(),
        "nothing ran: {:?}",
        watching.seen()
    );
}

#[tokio::test]
async fn a_bare_confirmation_is_no_yes_to_a_replacement() {
    let watching = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = theirs(&watching);

    let found = replacing_as(
        &ctx,
        MigrateAction::Act {
            mode: Mode::Replace,
            confirmed: true,
        },
    )
    .await
    .ok()
    .flatten();
    assert_eq!(
        found.map(|read| read.stance),
        Some(Stance::Pending),
        "asked with a confirmation it only says what it would stop"
    );
    assert!(
        watching.seen().is_empty(),
        "nothing ran: {:?}",
        watching.seen()
    );
}

/// A project holding nothing lemonfiber runs is somebody's own work.
#[tokio::test]
async fn an_unrelated_project_is_not_stood_in_place_of() {
    let images = Pulled::holding(vec![Pulled::image("a-database:17", 400, &["shop"])]);
    let engine =
        Reporting::holding(&["postgres"], Lifecycle::Running, Health::Healthy).belonging_to("shop");
    let ctx = over(engine, images, Source::External(project()));
    let found = replacing(&ctx, Some("00000000")).await.ok().flatten();
    let refused = found.as_ref().and_then(|read| read.refusal.clone());
    assert!(refused.is_some(), "somebody else's work is not replaced");
    let offered = found.map(|read| read.agreement).unwrap_or_default();
    assert!(offered.is_empty(), "nothing to agree to is no offer");
}

/// A container that would not stop leaves the stack half up, and a script has to be
/// able to tell that from a clean replacement.
#[tokio::test]
async fn a_container_that_would_not_stop_is_reported_as_still_running() {
    let stubborn = Arc::new(Recording::answering(Ok(refused("no such container"))));
    let ctx = theirs(&stubborn);

    let found = read_then_answered(&ctx).await;
    let still = found.as_ref().map(|read| read.still_running.clone());
    assert_eq!(still, Some(vec!["sonarr".to_owned()]), "{found:?}");
    let stopped = found.map(|read| read.stopped).unwrap_or_default();
    assert!(stopped.is_empty(), "{stopped:?}");
}
