//! Standing beside an existing setup.

use super::common::stack::project;
use super::{over, over_files, scratch, somebody_elses, standing, theirs};
use lemonfiber_core::app::{dispatch, Command, MigrateAction};
use lemonfiber_core::migration::mode::Mode;
use lemonfiber_core::reconfigure::Stance;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::SeedFs;
use std::sync::Arc;

#[tokio::test]
async fn standing_beside_unconfirmed_says_where_it_would_listen_and_writes_nothing() {
    let env = scratch("beside-rehearsed");
    let found = standing(&theirs("4.0.15", Some(env.to_path_buf())), false).await;
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Pending),
        "{found:?}"
    );
    let moved = found.map(|read| read.ports.len()).unwrap_or_default();
    assert!(moved > 0, "somewhere to listen was named");
    assert!(!env.exists(), "a rehearsal wrote {}", env.display());
}

#[tokio::test]
async fn confirming_writes_the_layered_file_and_records_where_it_is() {
    let env = scratch("beside-applied");
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.15",
        400,
        &["media"],
    )]);
    let files = Arc::new(SeedFs::keyed(None, None));
    let mut ctx = over_files(somebody_elses(), images, Arc::clone(&files));
    ctx.settings.env_file = Some(env.to_path_buf());

    let found = standing(&ctx, true).await;
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Applied),
        "{found:?}"
    );

    // Where it says it wrote, and what actually went through the filesystem, are two
    // facts; a report claiming a file it never wrote is the bug worth catching.
    let recorded = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(recorded.contains("LEMONFIBER_OVERLAY="), "{recorded}");

    let wrote = files.wrote();
    let layered = wrote
        .first()
        .map(|(_, contents)| contents.clone())
        .unwrap_or_default();
    assert!(layered.starts_with("services:"), "{layered}");
    assert!(layered.contains("sonarr:"), "{layered}");
}

/// A machine it could not read is one whose free ports it would be guessing at.
#[tokio::test]
async fn standing_beside_what_could_not_be_read_is_refused() {
    let images = Pulled::unreachable("no daemon here");
    let ctx = over(somebody_elses(), images, Source::External(project()));
    let found = standing(&ctx, true).await;
    let refused = found.and_then(|read| read.refusal);
    assert!(
        refused.is_some_and(|said| said.contains("could not be read")),
        "refused"
    );
}

/// Standing beside is the operator's explicit act, so nowhere to write it is reported
/// rather than shrugged off.
#[tokio::test]
async fn standing_beside_with_nowhere_to_write_says_so() {
    let asked = Command::Migrate(MigrateAction::Act {
        mode: Mode::Beside,
        confirmed: true,
    });
    let refused = dispatch(asked, &theirs("4.0.15", None)).await;
    assert!(refused.is_err(), "{refused:?}");
}

/// And where there is somewhere but it cannot be written to.
#[tokio::test]
async fn standing_beside_that_cannot_record_where_it_wrote_reports_the_failure() {
    let blocked = lemonfiber_fixtures::scratch::Scratch::named("beside");
    let _ = std::fs::write(&blocked, "not a directory");
    let asked = Command::Migrate(MigrateAction::Act {
        mode: Mode::Beside,
        confirmed: true,
    });
    let ctx = theirs("4.0.15", Some(blocked.join(".env")));
    let refused = dispatch(asked, &ctx).await;
    let _ = std::fs::remove_file(&blocked);
    assert!(refused.is_err(), "{refused:?}");
}
