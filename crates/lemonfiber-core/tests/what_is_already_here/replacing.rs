//! Standing in place of an existing setup.

use crate::common::stack::project;
use crate::{ctx, driven, over, replacing, somebody_elses, theirs};
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::Runner;
use lemonfiber_core::reconfigure::Stance;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{refused, spoke, Recording, Reporting};
use std::sync::Arc;

#[tokio::test]
async fn standing_in_place_unconfirmed_names_what_would_stop_and_stops_nothing() {
    let watching = Arc::new(Recording::answering(Ok(spoke(""))));
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.15",
        400,
        &["media"],
    )]);
    let ctx = driven(
        somebody_elses(),
        images,
        Source::External(project()),
        Arc::clone(&watching) as Arc<dyn Runner>,
    );

    let found = replacing(&ctx, false).await;
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Pending),
        "{found:?}"
    );
    let named = found.map(|read| read.would_stop).unwrap_or_default();
    assert_eq!(named, vec!["sonarr".to_owned()], "what would stop");
    assert!(
        watching.seen().is_empty(),
        "a rehearsal ran {:?}",
        watching.seen()
    );
}

#[tokio::test]
async fn confirming_stops_what_was_named_and_deletes_none_of_it() {
    let watching = Arc::new(Recording::answering(Ok(spoke(""))));
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.15",
        400,
        &["media"],
    )]);
    let ctx = driven(
        somebody_elses(),
        images,
        Source::External(project()),
        Arc::clone(&watching) as Arc<dyn Runner>,
    );

    let found = replacing(&ctx, true).await;
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

/// A project holding nothing lemonfiber runs is somebody's own work.
#[tokio::test]
async fn an_unrelated_project_is_not_stood_in_place_of() {
    let images = Pulled::holding(vec![Pulled::image("a-database:17", 400, &["shop"])]);
    let engine =
        Reporting::holding(&["postgres"], Lifecycle::Running, Health::Healthy).belonging_to("shop");
    let ctx = over(engine, images, Source::External(project()));
    let found = replacing(&ctx, true).await;
    let refused = found.and_then(|read| read.refusal);
    assert!(refused.is_some(), "somebody else's work is not replaced");
}

/// A container that would not stop leaves the stack half up, and a script has to be
/// able to tell that from a clean replacement.
#[tokio::test]
async fn a_container_that_would_not_stop_is_reported_as_still_running() {
    let stubborn = Arc::new(Recording::answering(Ok(refused("no such container"))));
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.15",
        400,
        &["media"],
    )]);
    let ctx = driven(
        somebody_elses(),
        images,
        Source::External(project()),
        Arc::clone(&stubborn) as Arc<dyn Runner>,
    );

    let found = replacing(&ctx, true).await;
    let still = found.as_ref().map(|read| read.still_running.clone());
    assert_eq!(still, Some(vec!["sonarr".to_owned()]), "{found:?}");
    let stopped = found.map(|read| read.stopped).unwrap_or_default();
    assert!(stopped.is_empty(), "{stopped:?}");
}
