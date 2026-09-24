//! Adopting an existing setup.

use crate::common::stack::project;
use crate::{adopting, ctx, mounting, over, scratch, somebody_elses, standing, theirs};
use lemonfiber_core::app::{dispatch, Command, MigrateAction};
use lemonfiber_core::migration::mode::Mode;
use lemonfiber_core::reconfigure::Stance;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{refused, Reporting};

#[tokio::test]
async fn a_database_a_later_version_wrote_is_refused_rather_than_opened() {
    let found = adopting(&theirs("9.9.9", None), true).await;
    let refused = found.as_ref().and_then(|read| read.refusal.clone());
    assert!(
        refused.is_some_and(|said| said.contains("later version")),
        "{found:?}"
    );
    assert_eq!(
        found.map(|read| read.stance),
        Some(Stance::Blocked),
        "refused rather than done"
    );
}

#[tokio::test]
async fn adopting_unconfirmed_says_what_it_would_do_and_writes_nothing() {
    let env = scratch("rehearsed");
    let found = adopting(&theirs("4.0.0", Some(env.clone())), false).await;
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Pending),
        "{found:?}"
    );
    assert!(!env.exists(), "a rehearsal wrote {}", env.display());
}

#[tokio::test]
async fn an_upgrade_names_the_service_and_where_to_back_it_up_before_confirming() {
    let env = scratch("named");
    let engine = mounting(somebody_elses(), &["/srv/media"]);
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.0",
        400,
        &["media"],
    )]);
    let mut ctx = over(engine, images, Source::External(project()));
    ctx.settings.env_file = Some(env);

    let found = adopting(&ctx, false).await;
    let upgrading = found
        .as_ref()
        .and_then(|read| read.upgrades.first().map(|one| one.service.clone()));
    assert_eq!(upgrading, Some("sonarr".to_owned()), "{found:?}");
    let paths = found.map(|read| read.back_up).unwrap_or_default();
    assert!(
        paths.iter().any(|path| path.contains("/srv/media")),
        "{paths:?}"
    );
}

#[tokio::test]
async fn confirming_records_the_project_lemonfiber_now_manages() {
    let env = scratch("adopted");
    let found = adopting(&theirs("4.0.15", Some(env.clone())), true).await;
    assert_eq!(
        found
            .as_ref()
            .map(|read| (read.stance, read.project.clone())),
        Some((Stance::Applied, Some("media".to_owned()))),
        "{found:?}"
    );
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(written.contains("LEMONFIBER_PROJECT=media"), "{written}");
}

#[tokio::test]
async fn a_machine_with_nothing_of_ours_on_it_has_nothing_to_adopt() {
    let images = Pulled::holding(Vec::new());
    let ctx = over(Reporting::absent(), images, Source::External(project()));
    let found = adopting(&ctx, true).await;
    let refused = found.and_then(|read| read.refusal);
    assert!(refused.is_some(), "nothing to take over is a refusal");
}

/// Adopting is the operator's explicit act, so nowhere to record it is reported rather
/// than shrugged off — an answer that quietly did not persist would leave them
/// believing lemonfiber manages a stack it does not.
#[tokio::test]
async fn adopting_with_nowhere_to_record_it_says_so_rather_than_claiming_it_worked() {
    let asked = Command::Migrate(MigrateAction::Act {
        mode: Mode::Adopt,
        confirmed: true,
    });
    let refused = dispatch(asked, &theirs("4.0.15", None)).await;
    assert!(refused.is_err(), "{refused:?}");
}

/// The same where there is somewhere but it cannot be written: a file standing where
/// the directory would go.
#[tokio::test]
async fn adopting_that_cannot_write_its_answer_reports_the_failure() {
    let blocked = std::env::temp_dir().join(format!("lemonfiber-blocked-{}", std::process::id()));
    let _ = std::fs::write(&blocked, "not a directory");
    let asked = Command::Migrate(MigrateAction::Act {
        mode: Mode::Adopt,
        confirmed: true,
    });
    let ctx = theirs("4.0.15", Some(blocked.join(".env")));
    let refused = dispatch(asked, &ctx).await;
    let _ = std::fs::remove_file(&blocked);
    assert!(refused.is_err(), "{refused:?}");
}
