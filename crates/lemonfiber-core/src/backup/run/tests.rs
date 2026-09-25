use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::support::Reporting;

use super::{
    backup, capture, existing as capture_existing, Report, Taking, NOT_MEASURED, NOT_WRITTEN,
    NOWHERE_TO_KEEP, NO_ROOM, STILL_RUNNING,
};
use crate::app::fixtures::{keeping, paths, FakeArchive};
use crate::app::Ctx;
use crate::archive::{Fault, Space};
use crate::backup::{Existing, Retention, Scope};
use crate::ports::docker::{Health, Lifecycle};

async fn capturing(archive: &FakeArchive) -> Result<Report, Box<crate::error::Problem>> {
    capture(
        &paths(),
        Scope::WholeStack,
        "0.3.0",
        "2026-07-30T00:00:00Z",
        "/srv/media",
        Taking::of(Retention::keeping(2), false),
        archive,
    )
    .await
}

/// The same capture, said rather than taken.
async fn rehearsing(
    archive: &FakeArchive,
    retention: Retention,
) -> Result<Report, Box<crate::error::Problem>> {
    capture(
        &paths(),
        Scope::WholeStack,
        "0.3.0",
        "2026-07-30T00:00:00Z",
        "/srv/media",
        Taking::of(retention, true),
        archive,
    )
    .await
}

/// A whole-stack archive on disk, named as a capture would name it, taken at
/// the given time.
fn full(at: &str) -> Existing {
    Existing {
        name: format!("lemonfiber-full-{at}.tar.gz"),
        created_at: at.to_owned(),
    }
}

/// A single-service archive on disk, for `service`, taken at the given time.
fn service(service: &str, at: &str) -> Existing {
    Existing {
        name: format!("lemonfiber-{service}-{at}.tar.gz"),
        created_at: at.to_owned(),
    }
}

/// The name a whole-stack capture at the stamp `capturing` uses is written
/// under — the fresh archive retention must never prune.
const FRESH: &str = "lemonfiber-full-2026-07-30T00-00-00Z.tar.gz";

#[tokio::test]
async fn a_rehearsed_capture_names_the_archive_and_writes_nothing() {
    // The whole of the claim: the destination, the size and what retention would
    // drop are all settled before the write, so the report is the real one and the
    // archive directory is untouched.
    let archive = FakeArchive::keeping_backups(&[
        ("lemonfiber-full-2026-07-01T00-00-00Z.tar.gz", "2026-07-01"),
        ("lemonfiber-full-2026-07-02T00-00-00Z.tar.gz", "2026-07-02"),
    ]);
    let report = rehearsing(&archive, Retention::keeping(1))
        .await
        .map_err(|problem| *problem);

    assert_eq!(
        report
            .as_ref()
            .ok()
            .map(|report| (report.rehearsed, report.path.clone())),
        Some((true, paths().backups().join(FRESH)))
    );
    assert_eq!(
        report.map(|report| report.pruned),
        Ok(vec![
            "lemonfiber-full-2026-07-01T00-00-00Z.tar.gz".to_owned()
        ]),
        "a rehearsal that only counted them would not say which is about to go"
    );
    assert!(archive.writes().is_empty(), "the archive was written");
    assert!(archive.removes().is_empty(), "an older archive was pruned");
}

#[tokio::test]
async fn a_capture_writes_the_archive_under_the_backups_directory_and_reports_it() {
    let archive = FakeArchive::roomy();
    let report = capturing(&archive).await;

    // The archive lands under the backups directory, named for the scope and
    // the stamp with its colons made filename-safe; the report is sensitive.
    assert_eq!(
        report.map(|report| (report.path, report.sensitive, report.pruned)),
        Ok((
            PathBuf::from(format!("/data/lemonfiber/backups/{FRESH}")),
            true,
            Vec::new()
        ))
    );
    assert_eq!(archive.writes().len(), 1, "the archive was written once");
}

#[tokio::test]
async fn a_single_service_capture_is_named_for_its_service() {
    let archive = FakeArchive::roomy();
    let report = capture(
        &paths(),
        Scope::Service {
            name: "sonarr".to_owned(),
        },
        "0.3.0",
        "2026-07-30T00:00:00Z",
        "/srv/media",
        Taking::of(Retention::keeping(2), false),
        &archive,
    )
    .await
    .map_err(|problem| *problem);
    assert_eq!(
        report.map(|report| report.path),
        Ok(PathBuf::from(
            "/data/lemonfiber/backups/lemonfiber-sonarr-2026-07-30T00-00-00Z.tar.gz"
        ))
    );
}

#[tokio::test]
async fn a_capture_prunes_the_oldest_backups_past_the_keep_count() {
    let archive = FakeArchive {
        existing: Ok(vec![
            full("2026-07-02"),
            full("2026-07-01"),
            full("2026-07-03"),
        ]),
        ..FakeArchive::roomy()
    };
    let report = capturing(&archive).await;
    // Keeping two of three prunes only the eldest, whatever order it was listed.
    assert_eq!(
        report.map(|report| report.pruned),
        Ok(vec!["lemonfiber-full-2026-07-01.tar.gz".to_owned()])
    );
    assert_eq!(
        archive.removes(),
        vec!["lemonfiber-full-2026-07-01.tar.gz".to_owned()]
    );
}

#[tokio::test]
async fn retention_keeps_each_scope_on_its_own_count() {
    // A whole-stack capture prunes only whole-stack archives; a single service's
    // targeted backup is not swept away because later whole-stack ones are newer.
    let archive = FakeArchive {
        existing: Ok(vec![
            full("2026-07-01"),
            service("sonarr", "2026-07-02"),
            full("2026-07-29"),
        ]),
        ..FakeArchive::roomy()
    };
    let report = capture(
        &paths(),
        Scope::WholeStack,
        "0.3.0",
        "2026-07-30T00:00:00Z",
        "/srv/media",
        Taking::of(Retention::keeping(1), false),
        &archive,
    )
    .await
    .map_err(|problem| *problem);
    // Keep one whole-stack of the two older ones (plus the fresh one), and never
    // the sonarr archive, which belongs to a different scope's count.
    assert_eq!(
        report.map(|report| report.pruned),
        Ok(vec!["lemonfiber-full-2026-07-01.tar.gz".to_owned()])
    );
}

#[tokio::test]
async fn the_archive_just_written_is_never_pruned() {
    // A clock running behind could stamp the fresh archive older than the ones
    // already there; retention must still keep it, so the guard excludes it by
    // name whatever its recorded time.
    let archive = FakeArchive {
        existing: Ok(vec![
            Existing {
                name: FRESH.to_owned(),
                created_at: "2000-01-01".to_owned(),
            },
            full("2026-07-29"),
        ]),
        ..FakeArchive::roomy()
    };
    // Keeping one of two would prune the eldest — which the skewed stamp makes
    // the fresh archive — so the guard, not the count, is what saves it.
    let report = capture(
        &paths(),
        Scope::WholeStack,
        "0.3.0",
        "2026-07-30T00:00:00Z",
        "/srv/media",
        Taking::of(Retention::keeping(1), false),
        &archive,
    )
    .await
    .map_err(|problem| *problem);
    assert_eq!(
        report.map(|report| report.pruned),
        Ok(Vec::new()),
        "the freshest capture is kept even when its stamp sorts oldest"
    );
    assert!(
        !archive.removes().contains(&FRESH.to_owned()),
        "the archive just written was never removed"
    );
}

#[tokio::test]
async fn a_capture_that_would_not_fit_is_refused_before_writing() {
    let archive = FakeArchive {
        space: Ok(Space {
            needed: 1_000,
            available: 500,
        }),
        ..FakeArchive::roomy()
    };
    let refusal = capturing(&archive).await.err().map(|problem| problem.code);
    assert_eq!(refusal, Some(NO_ROOM));
    assert!(archive.writes().is_empty(), "nothing was written");
}

#[tokio::test]
async fn a_capture_whose_room_cannot_be_measured_is_refused() {
    let archive = FakeArchive {
        space: Err(Fault::new("no such volume")),
        ..FakeArchive::roomy()
    };
    let refusal = capturing(&archive).await.err().map(|problem| problem.code);
    assert_eq!(refusal, Some(NOT_MEASURED));
    assert!(archive.writes().is_empty());
}

#[tokio::test]
async fn a_capture_whose_archive_cannot_be_written_is_reported() {
    let archive = FakeArchive {
        write: Err(Fault::new("read-only filesystem")),
        ..FakeArchive::roomy()
    };
    let refusal = capturing(&archive).await.err().map(|problem| problem.code);
    assert_eq!(refusal, Some(NOT_WRITTEN));
}

#[tokio::test]
async fn a_capture_still_succeeds_when_the_older_backups_cannot_be_listed() {
    // Retention is best-effort: an unreadable backups directory leaves the
    // pruning for next time, it does not undo a capture that has succeeded.
    let archive = FakeArchive {
        existing: Err(Fault::new("permission denied")),
        ..FakeArchive::roomy()
    };
    let report = capturing(&archive).await;
    assert_eq!(report.map(|report| report.pruned), Ok(Vec::new()));
    assert!(archive.removes().is_empty());
}

#[tokio::test]
async fn an_old_backup_that_will_not_delete_is_left_and_the_capture_still_succeeds() {
    let archive = FakeArchive {
        existing: Ok(vec![full("2026-07-01"), full("2026-07-02")]),
        remove: Err(Fault::new("busy")),
        ..FakeArchive::roomy()
    };
    // Keeping one of two prunes the eldest, but the delete failed, so the report
    // names nothing pruned even though the removal was attempted.
    let report = capture(
        &paths(),
        Scope::WholeStack,
        "0.3.0",
        "2026-07-30T00:00:00Z",
        "/srv/media",
        Taking::of(Retention::keeping(1), false),
        &archive,
    )
    .await
    .map_err(|problem| *problem);
    assert_eq!(
        report.map(|report| report.pruned),
        Ok(Vec::new()),
        "a failed delete is not reported as pruned"
    );
    assert_eq!(
        archive.removes(),
        vec!["lemonfiber-full-2026-07-01.tar.gz".to_owned()],
        "it was attempted"
    );
}

/// A run whose engine answers and reports nothing running.
fn stopped() -> Ctx {
    crate::test_support::a_context()
        .engine(Arc::new(Reporting::holding(
            &["sonarr"],
            Lifecycle::Exited,
            Health::None,
        )))
        .build()
}

/// The same run, keeping its archives through `vault`.
fn a_stopped_run(vault: &Arc<FakeArchive>) -> Ctx {
    keeping(stopped(), vault)
}

#[tokio::test]
async fn a_run_with_nowhere_to_keep_an_archive_refuses_rather_than_guessing_a_path() {
    let refusal = backup(&stopped(), None)
        .await
        .err()
        .map(|problem| problem.code);
    assert_eq!(refusal, Some(NOWHERE_TO_KEEP));
}

#[tokio::test]
async fn a_capture_of_the_whole_stack_lands_in_the_backups_directory() {
    let vault = Arc::new(FakeArchive::roomy());
    let report = backup(&a_stopped_run(&vault), None)
        .await
        .map_err(|problem| problem.code);
    assert_eq!(report.map(|report| report.scope), Ok(Scope::WholeStack));
    let written = vault.writes();
    assert_eq!(written.len(), 1, "one archive was written");
    assert!(
        written
            .first()
            .is_some_and(|path| path.starts_with("/data/lemonfiber/backups")),
        "{written:?}"
    );
}

#[tokio::test]
async fn a_capture_of_one_service_records_that_scope() {
    let vault = Arc::new(FakeArchive::roomy());
    let report = backup(&a_stopped_run(&vault), Some("sonarr".to_owned()))
        .await
        .map_err(|problem| problem.code);
    assert_eq!(
        report.map(|report| report.scope),
        Ok(Scope::Service {
            name: "sonarr".to_owned()
        })
    );
}

#[tokio::test]
async fn a_capture_is_refused_while_the_services_may_be_writing() {
    // A copy of a live database is the corruption a backup exists to prevent, so
    // a browser cannot ask for the capture a shell is not allowed either.
    let vault = Arc::new(FakeArchive::roomy());
    let running = crate::test_support::a_context()
        .engine(Arc::new(Reporting::holding(
            &["sonarr"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .build();
    let refusal = backup(&keeping(running, &vault), None)
        .await
        .err()
        .map(|problem| problem.code);
    assert_eq!(refusal, Some(STILL_RUNNING));
    assert!(vault.writes().is_empty(), "nothing was written");
}

/// A machine whose *other* setup — not lemonfiber's — is in the given state.
///
/// The project matters as much as the lifecycle here: a capture taken before a
/// takeover has to prove that setup still, and lemonfiber's own project is one
/// nothing runs under yet.
fn theirs(lifecycle: Lifecycle) -> Ctx {
    crate::test_support::a_context()
        .engine(Arc::new(
            Reporting::holding(&["sonarr"], lifecycle, Health::None).belonging_to("media"),
        ))
        .build()
}

/// The trees a setup being taken over keeps its data in.
fn trees() -> Vec<String> {
    vec!["/srv/their-media".to_owned()]
}

#[tokio::test]
async fn a_capture_before_a_takeover_records_the_setup_and_the_trees_it_covers() {
    let vault = Arc::new(FakeArchive::roomy());
    let ctx = keeping(theirs(Lifecycle::Exited), &vault);
    let report = capture_existing(&ctx, "media", &trees())
        .await
        .map_err(|problem| problem.code);

    assert_eq!(
        report.as_ref().map(|report| report.scope.clone()),
        Ok(Scope::existing("media", &trees()))
    );
    // Named for the project, so taking over two setups in turn does not leave the
    // second capture pruning the first.
    assert!(
        report.is_ok_and(|report| report
            .path
            .to_string_lossy()
            .contains("lemonfiber-existing-media-")),
        "the archive is not named for the setup it holds"
    );
}

/// The proof is about *their* project, so their running setup refuses the capture.
#[tokio::test]
async fn a_capture_before_a_takeover_is_refused_while_that_setup_is_still_running() {
    let vault = Arc::new(FakeArchive::roomy());
    let ctx = keeping(theirs(Lifecycle::Running), &vault);
    let refusal = capture_existing(&ctx, "media", &trees())
        .await
        .err()
        .map(|problem| problem.code);

    assert_eq!(refusal, Some(STILL_RUNNING));
    assert!(vault.writes().is_empty(), "it captured a live database");
}

/// And it is *only* about their project: lemonfiber's own containers running says
/// nothing about whether the setup being captured is writing to its databases.
#[tokio::test]
async fn a_capture_before_a_takeover_does_not_ask_about_lemonfibers_own_project() {
    let vault = Arc::new(FakeArchive::roomy());
    let mixed = crate::test_support::a_context()
        .engine(Arc::new(
            Reporting::holding(&["sonarr"], Lifecycle::Running, Health::None).alongside(
                Reporting::holding(&["radarr"], Lifecycle::Exited, Health::None)
                    .belonging_to("media"),
            ),
        ))
        .build();

    let report = capture_existing(&keeping(mixed, &vault), "media", &trees()).await;
    assert!(report.is_ok(), "{report:?}");
}

#[tokio::test]
async fn a_capture_before_a_takeover_with_nowhere_to_keep_it_refuses_rather_than_guessing() {
    let refusal = capture_existing(&theirs(Lifecycle::Exited), "media", &trees())
        .await
        .err()
        .map(|problem| problem.code);
    assert_eq!(refusal, Some(NOWHERE_TO_KEEP));
}
