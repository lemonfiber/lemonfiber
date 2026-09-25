use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use super::{grounded, waited, NO_DATA_LOCATION};
use crate::app::Ctx;
use crate::config::Settings;
use crate::ports::filesystem::{Presence, Volume};
use crate::stack::compose::Action;
use crate::test_support::a_context;

/// A drive answering each reading with the next one a test scripted, then staying
/// on the last one once the script runs out.
struct Drive {
    readings: Vec<Presence>,
    cursor: AtomicUsize,
}

#[async_trait]
impl Volume for Drive {
    async fn presence(&self, _path: &Path) -> Presence {
        let at = self.cursor.fetch_add(1, Ordering::SeqCst);
        self.readings
            .get(at)
            .or_else(|| self.readings.last())
            .copied()
            .unwrap_or(Presence::Gone)
    }
}

/// A context whose data location answers with the given readings in turn.
fn ctx_reading(readings: Vec<Presence>) -> Ctx {
    a_context()
        .settings(Settings {
            data_root: Some(PathBuf::from("/Volumes/media")),
            ..Settings::default()
        })
        .build()
        .with_volume(Arc::new(Drive {
            readings,
            cursor: AtomicUsize::new(0),
        }))
}

/// The wait, driven with a handful of looks and no interval at all.
async fn over(ctx: &Ctx, action: &Action) -> Result<(), Box<crate::error::Problem>> {
    waited(ctx, action, 3, Duration::ZERO).await
}

#[tokio::test]
async fn a_location_that_is_there_holds_nothing_up() {
    let ctx = ctx_reading(vec![Presence::On(7)]);
    assert!(grounded(&ctx, &Action::Up).await.is_ok());
}

#[tokio::test]
async fn a_location_that_turns_up_late_is_waited_for_rather_than_refused() {
    // The drive that spins up a few seconds after the login. Refusing on the
    // first reading would make every restart of a machine with an external
    // library a coin toss.
    let ctx = ctx_reading(vec![Presence::Gone, Presence::Gone, Presence::On(7)]);
    assert!(over(&ctx, &Action::Up).await.is_ok());
}

#[tokio::test]
async fn a_location_that_never_appears_stops_the_start_and_says_why() {
    // The whole point: the engine would otherwise make the directory on the
    // system disk and file a library into it while the real one is offline.
    let ctx = ctx_reading(vec![Presence::Gone]);

    let refused = over(&ctx, &Action::Up).await.err();

    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(NO_DATA_LOCATION)
    );
    assert!(
        refused
            .as_ref()
            .is_some_and(|problem| problem.summary.contains("/Volumes/media")),
        "it names the location that is missing"
    );
    assert!(
        refused.is_some_and(|problem| problem.meaning.contains("system disk")),
        "and what starting anyway would have done"
    );
}

#[tokio::test]
async fn a_reading_that_could_not_be_taken_is_not_an_absence() {
    // A permission error or an interrupted call says nothing about whether the
    // drive is there, and stopping a start over one would be stopping it over a
    // hiccup — the judgement the running guard makes, for the same reason.
    let ctx = ctx_reading(vec![Presence::Unknown]);
    assert!(grounded(&ctx, &Action::Up).await.is_ok());
}

#[tokio::test]
async fn nothing_but_a_start_is_held_to_it() {
    // A teardown blocked because a drive had been unplugged would refuse to tidy
    // up after exactly the accident it is being run about.
    let ctx = ctx_reading(vec![Presence::Gone]);
    for action in [
        Action::Down,
        Action::Stop(Vec::new()),
        Action::Restart(Vec::new()),
        Action::Pull,
        Action::Config,
    ] {
        assert!(
            over(&ctx, &action).await.is_ok(),
            "{action:?} does not write into the data location"
        );
    }
}

#[tokio::test]
async fn starting_named_services_is_held_to_it_too() {
    // They write into the same location the whole form does, so the narrower
    // request is the same hazard with fewer containers in it.
    let ctx = ctx_reading(vec![Presence::Gone]);
    assert!(over(&ctx, &Action::Start(vec!["sonarr".to_owned()]))
        .await
        .is_err());
}

#[tokio::test]
async fn a_machine_with_no_data_location_has_nothing_to_prove() {
    let ctx = a_context()
        .settings(Settings::default())
        .build()
        .with_volume(Arc::new(Drive {
            readings: vec![Presence::Gone],
            cursor: AtomicUsize::new(0),
        }));
    assert!(over(&ctx, &Action::Up).await.is_ok());
}
