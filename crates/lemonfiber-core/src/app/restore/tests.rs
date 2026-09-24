use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_fixtures::support::Reporting;

use super::{
    inspect, restore, run, Consent, Kept, CORRUPT, INCOMPATIBLE, MOVED_ON, NEEDS_REPOINT,
    NOT_KEPT_HERE, NOT_OURS, NOT_REPOINTED, NOT_RESTORED, NOWHERE_KEPT, STILL_RUNNING, TOO_NEW,
    UNSAFE,
};
use crate::app::fixtures::{keeping, paths, scratch, FakeArchive, CURRENT};
use crate::app::Ctx;
use crate::archive::{Archiving, Fault};
use crate::backup::{Member, Scope, SCHEMA};
use crate::config::paths::Paths;
use crate::config::Settings;
use crate::ports::docker::{Health, Lifecycle};

fn archive() -> PathBuf {
    PathBuf::from("/data/lemonfiber/backups/lemonfiber-full-2026-07-30T00-00-00Z.tar.gz")
}

/// Restore against a machine at version 0.3.0 and data root `/srv/media`,
/// accepting no re-point.
async fn restoring(reader: &FakeArchive) -> Result<super::Report, Box<super::super::Problem>> {
    restore(
        &archive(),
        &paths(),
        "0.3.0",
        SCHEMA,
        Path::new("/srv/media"),
        false,
        reader,
    )
    .await
}

async fn inspecting(
    reader: &FakeArchive,
    current_root: &str,
) -> Result<super::Preview, Box<super::super::Problem>> {
    inspect(&archive(), "0.3.0", SCHEMA, Path::new(current_root), reader).await
}

#[tokio::test]
async fn a_compatible_archive_is_previewed_with_its_contents() {
    let reader = FakeArchive::holding(CURRENT, SCHEMA);
    let preview = inspecting(&reader, "/srv/media").await;
    assert_eq!(
        preview.map(|preview| (
            preview.manifest.scope,
            preview.downgrade,
            preview.relocation
        )),
        Ok((Scope::WholeStack, false, None))
    );
}

#[tokio::test]
async fn a_restore_unpacks_the_archive_and_reports_what_it_restored() {
    let reader = FakeArchive::holding("0.2.0", SCHEMA);
    let report = restoring(&reader).await;
    assert_eq!(
        report.map(|report| (report.scope, report.from_version, report.relocated)),
        Ok((Scope::WholeStack, "0.2.0".to_owned(), None))
    );
    assert_eq!(
        reader.extractions(),
        vec![archive()],
        "the archive was unpacked once"
    );
}

#[tokio::test]
async fn a_corrupt_archive_is_refused_before_anything_is_overwritten() {
    let reader = FakeArchive {
        manifest: Err(Fault::new("unexpected end of archive")),
        ..FakeArchive::holding(CURRENT, SCHEMA)
    };
    let refusal = restoring(&reader).await.err().map(|problem| problem.code);
    assert_eq!(refusal, Some(CORRUPT));
    assert!(reader.extractions().is_empty(), "nothing was unpacked");
}

#[tokio::test]
async fn a_newer_archive_is_refused_with_the_version_gap() {
    let reader = FakeArchive::holding("0.4.0", SCHEMA);
    let refusal = restoring(&reader).await.err().map(|problem| problem.code);
    assert_eq!(refusal, Some(TOO_NEW));
}

#[tokio::test]
async fn an_archive_in_an_unreadable_format_is_refused() {
    let reader = FakeArchive::holding(CURRENT, SCHEMA + 1);
    let refusal = restoring(&reader).await.err().map(|problem| problem.code);
    assert_eq!(refusal, Some(INCOMPATIBLE));
}

/// An archive of a setup lemonfiber does not manage is refused, and says where
/// its trees came from so the operator can put them back themselves.
///
/// The refusal is not a complaint about the archive — it is a good capture of
/// exactly what it says it holds. What lemonfiber will not do is write it back
/// into directories that were never its to write to.
#[tokio::test]
async fn an_archive_of_a_setup_we_do_not_manage_is_refused_and_nothing_is_unpacked() {
    let mut reader = FakeArchive::holding(CURRENT, SCHEMA);
    if let Ok(manifest) = &mut reader.manifest {
        manifest.scope = Scope::existing("media", &["/srv/their-media".to_owned()]);
    }

    let refusal = restoring(&reader).await.err();
    assert_eq!(refusal.as_ref().map(|problem| problem.code), Some(NOT_OURS));
    assert!(
        refusal
            .and_then(|problem| problem.detail.clone())
            .is_some_and(|said| said.contains("/srv/their-media")),
        "the refusal did not say where to put it back"
    );
    assert!(reader.extractions().is_empty(), "it unpacked it anyway");
}

#[tokio::test]
async fn an_archive_whose_member_would_escape_is_refused() {
    let mut reader = FakeArchive::holding(CURRENT, SCHEMA);
    if let Ok(manifest) = &mut reader.manifest {
        manifest.members.push(Member {
            archive_path: "../../etc/passwd".to_owned(),
            label: "hostile".to_owned(),
        });
    }
    let refusal = restoring(&reader).await.err().map(|problem| problem.code);
    assert_eq!(refusal, Some(UNSAFE));
    assert!(reader.extractions().is_empty());
}

#[tokio::test]
async fn a_much_older_archive_is_previewed_as_a_downgrade_and_still_restores() {
    // A whole major version behind is allowed with a warning; the preview
    // carries the warning and the restore proceeds.
    let reader = FakeArchive::holding("1.4.0", SCHEMA);
    let preview = inspect(
        &archive(),
        "2.0.0",
        SCHEMA,
        Path::new("/srv/media"),
        &reader,
    )
    .await
    .map_err(|problem| *problem);
    assert_eq!(preview.map(|preview| preview.downgrade), Ok(true));

    let report = restore(
        &archive(),
        &paths(),
        "2.0.0",
        SCHEMA,
        Path::new("/srv/media"),
        false,
        &reader,
    )
    .await;
    assert!(report.is_ok(), "a warned downgrade still restores");
}

#[tokio::test]
async fn a_different_data_root_is_reported_by_inspect_rather_than_refused() {
    let reader = FakeArchive::holding(CURRENT, SCHEMA);
    let preview = inspecting(&reader, "/mnt/library").await;
    assert_eq!(
        preview.map(|preview| preview.relocation.map(|move_| (move_.was, move_.now))),
        Ok(Some(("/srv/media".to_owned(), "/mnt/library".to_owned())))
    );
}

#[tokio::test]
async fn a_restore_onto_a_different_data_root_waits_for_the_re_point_to_be_accepted() {
    let reader = FakeArchive::holding(CURRENT, SCHEMA);
    let refusal = restore(
        &archive(),
        &paths(),
        "0.3.0",
        SCHEMA,
        Path::new("/mnt/library"),
        false,
        &reader,
    )
    .await
    .err()
    .map(|problem| problem.code);
    assert_eq!(refusal, Some(NEEDS_REPOINT));
    assert!(reader.extractions().is_empty(), "nothing was unpacked");
}

#[tokio::test]
async fn a_restore_onto_a_different_data_root_proceeds_once_re_pointing_is_accepted() {
    let reader = FakeArchive::holding(CURRENT, SCHEMA);
    let report = restore(
        &archive(),
        &paths(),
        "0.3.0",
        SCHEMA,
        Path::new("/mnt/library"),
        true,
        &reader,
    )
    .await
    .map_err(|problem| *problem);
    assert_eq!(
        report.map(|report| report.relocated.map(|move_| move_.now)),
        Ok(Some("/mnt/library".to_owned())),
        "the restore records the data root it re-pointed to"
    );
    assert_eq!(reader.extractions(), vec![archive()]);
}

#[tokio::test]
async fn a_restore_whose_extraction_fails_is_reported() {
    let reader = FakeArchive {
        extract: Err(Fault::new("read-only filesystem")),
        ..FakeArchive::holding(CURRENT, SCHEMA)
    };
    let refusal = restoring(&reader).await.err().map(|problem| problem.code);
    assert_eq!(refusal, Some(NOT_RESTORED));
}

/// The name a whole-stack archive is kept under.
const KEPT: &str = "lemonfiber-full-2026-07-30T00-00-00Z.tar.gz";

/// A run whose engine answers, reporting nothing running, whose data root is the
/// one the archives were taken against.
fn stopped() -> Ctx {
    crate::test_support::a_context()
        .engine(Arc::new(Reporting::holding(
            &["sonarr"],
            Lifecycle::Exited,
            Health::None,
        )))
        .settings(Settings {
            data_root: Some(PathBuf::from("/srv/media")),
            ..Settings::default()
        })
        .build()
}

/// The same run, keeping its archives through `vault`.
fn a_stopped_run(vault: &Arc<FakeArchive>) -> Ctx {
    keeping(stopped(), vault)
}

#[tokio::test]
async fn a_run_with_nowhere_it_keeps_archives_has_none_to_read() {
    let refusal = run(
        &stopped(),
        &Kept::Named(KEPT.to_owned()),
        false,
        &Consent::List,
    )
    .await
    .err()
    .map(|problem| problem.code);
    assert_eq!(refusal, Some(NOWHERE_KEPT));
}

#[tokio::test]
async fn a_name_that_is_a_path_is_refused_rather_than_followed() {
    // The server runs as the operator, so a path it accepted is a path it can
    // read. Only a file in the backups directory is one of the backups.
    let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
    let ctx = a_stopped_run(&vault);
    for name in ["../../etc/passwd", "older/full.tar.gz", ""] {
        let refusal = run(
            &ctx,
            &Kept::Named(name.to_owned()),
            false,
            &Consent::Standing,
        )
        .await
        .err()
        .map(|problem| problem.code);
        assert_eq!(refusal, Some(NOT_KEPT_HERE), "{name}");
    }
    assert!(vault.extractions().is_empty(), "nothing was unpacked");
}

#[tokio::test]
async fn an_unconfirmed_restore_lists_what_it_would_overwrite_and_touches_nothing() {
    let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
    let restoration = run(
        &a_stopped_run(&vault),
        &Kept::Named(KEPT.to_owned()),
        false,
        &Consent::List,
    )
    .await
    .map_err(|problem| problem.code);
    assert_eq!(
        restoration.map(|said| (said.would.manifest.scope, said.done.is_some())),
        Ok((Scope::WholeStack, false))
    );
    assert!(vault.extractions().is_empty(), "nothing was unpacked");
}

#[tokio::test]
async fn a_confirmed_restore_reads_the_archive_by_that_name_from_the_backups_directory() {
    let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
    let restoration = run(
        &a_stopped_run(&vault),
        &Kept::Named(KEPT.to_owned()),
        false,
        &Consent::Standing,
    )
    .await
    .map_err(|problem| problem.code);
    assert_eq!(restoration.map(|said| said.done.is_some()), Ok(true));
    assert_eq!(
        vault.extractions(),
        vec![PathBuf::from(format!("/data/lemonfiber/backups/{KEPT}"))]
    );
}

#[tokio::test]
async fn an_archive_named_by_path_is_read_from_where_it_was_named() {
    // What a shell has and a browser does not: a filesystem in front of it.
    let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
    let elsewhere = PathBuf::from("/mnt/usb/lemonfiber-full.tar.gz");
    let restoration = run(
        &a_stopped_run(&vault),
        &Kept::At(elsewhere.clone()),
        false,
        &Consent::Standing,
    )
    .await
    .map_err(|problem| problem.code);
    assert_eq!(restoration.map(|said| said.done.is_some()), Ok(true));
    assert_eq!(vault.extractions(), vec![elsewhere]);
}

#[tokio::test]
async fn a_restore_is_refused_while_the_services_may_be_writing() {
    let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
    let running = crate::test_support::a_context()
        .engine(Arc::new(Reporting::holding(
            &["sonarr"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(Settings {
            data_root: Some(PathBuf::from("/srv/media")),
            ..Settings::default()
        })
        .build();
    let refusal = run(
        &keeping(running, &vault),
        &Kept::Named(KEPT.to_owned()),
        false,
        &Consent::Standing,
    )
    .await
    .err()
    .map(|problem| problem.code);
    assert_eq!(refusal, Some(STILL_RUNNING));
    assert!(vault.extractions().is_empty(), "nothing was unpacked");
}

/// A run keeping its archives in a real directory, so the re-point that follows a
/// restore has an environment file it can actually write, whose own library is at
/// `root` — which is what a listing's re-point is derived from, and what moves
/// between one request and the next.
fn rooted_at(dir: &Path, root: &str, vault: &Arc<FakeArchive>) -> Ctx {
    crate::test_support::a_context()
        .engine(Arc::new(Reporting::holding(
            &["sonarr"],
            Lifecycle::Exited,
            Health::None,
        )))
        .settings(Settings {
            data_root: Some(PathBuf::from(root)),
            ..Settings::default()
        })
        .build()
        .keeping(Archiving {
            paths: Paths::at(dir, dir),
            vault: Arc::clone(vault) as Arc<dyn crate::archive::Vault>,
        })
}

/// The archive this machine keeps, by name.
fn kept() -> Kept {
    Kept::Named(KEPT.to_owned())
}

/// What the listing this run makes names itself, or nothing where it refused.
async fn as_listed(ctx: &Ctx) -> String {
    run(ctx, &kept(), true, &Consent::List)
        .await
        .ok()
        .map(|said| said.would.agreement)
        .unwrap_or_default()
}

/// An archive's own account of itself, covering `scope`, carrying credentials
/// or not, and holding `members`.
fn an_archive(scope: Scope, sensitive: bool, members: Vec<Member>) -> crate::backup::Manifest {
    crate::backup::Manifest {
        schema: SCHEMA,
        product_version: CURRENT.to_owned(),
        created_at: "2026-07-30T00:00:00Z".to_owned(),
        data_root: "/srv/media".to_owned(),
        scope,
        sensitive,
        members,
    }
}

/// A listing names what an operator reads in it, so an archive that would read
/// differently names itself differently.
///
/// One thing at a time, each of them something the operator is shown before
/// deciding: what the archive covers, whether it carries credentials, what is
/// inside it, and whether restoring it is a step backwards. Any of these
/// changing between the listing and the yes is a different restore being agreed
/// to under the same name.
#[test]
fn a_listing_names_what_an_operator_reads_in_it() {
    let whole = an_archive(Scope::WholeStack, true, Vec::new());
    let name = super::agreement(&whole, false, None);

    for (differing, said) in [
        (
            an_archive(
                Scope::Service {
                    name: "sonarr".to_owned(),
                },
                true,
                Vec::new(),
            ),
            "what it covers",
        ),
        (
            an_archive(
                Scope::existing("media", &["/srv/their-media".to_owned()]),
                true,
                Vec::new(),
            ),
            "a setup lemonfiber does not manage is not the whole stack",
        ),
        (
            an_archive(Scope::WholeStack, false, Vec::new()),
            "whether it carries credentials",
        ),
        (
            an_archive(
                Scope::WholeStack,
                true,
                vec![Member {
                    archive_path: "config/sonarr".to_owned(),
                    label: "Sonarr's configuration".to_owned(),
                }],
            ),
            "what is inside it",
        ),
    ] {
        assert_ne!(super::agreement(&differing, false, None), name, "{said}");
    }
    assert_ne!(
        super::agreement(&whole, true, None),
        name,
        "whether it is a step backwards"
    );
}

/// A yes is spent on the listing it was read in, and on no other.
///
/// The sequence a browser makes, which a shell cannot: the archive is listed, the
/// operator reads that restoring it would re-point their library onto
/// `/mnt/library` and agrees to that, and between the two requests this machine's
/// data root moves. The second request is the same request in every field — the
/// same archive, the same acceptance of a re-point — and the re-point it would
/// now carry out is to somewhere the operator has never seen. Nothing in the
/// answer would say so afterwards, because the listing that comes back with a
/// finished restore is the second one.
///
/// All three legs are asserted, because a refusal on its own proves only that
/// something failed: the listing really did move, the yes given for the old one
/// is refused, and the very same request naming the listing that stands now is
/// carried out.
#[tokio::test]
async fn a_yes_is_not_spent_on_a_listing_that_moved_between_the_two_requests() {
    let dir = scratch("restore-moved-on");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        std::fs::create_dir_all(&dir).is_ok(),
        "the scratch was made"
    );
    let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));

    let read = as_listed(&rooted_at(&dir, "/mnt/library", &vault)).await;
    let moved = rooted_at(&dir, "/mnt/elsewhere", &vault);
    let stands = as_listed(&moved).await;
    assert_ne!(read, stands, "the same archive now re-points elsewhere");

    let refusal = run(
        &moved,
        &kept(),
        true,
        &Consent::Given {
            listing: read.clone(),
        },
    )
    .await
    .err()
    .map(|problem| problem.code);
    assert_eq!(refusal, Some(MOVED_ON));
    assert!(vault.extractions().is_empty(), "nothing was unpacked");

    let carried = run(&moved, &kept(), true, &Consent::Given { listing: stands })
        .await
        .map_err(|problem| problem.code)
        .map(|said| said.done.and_then(|done| done.relocated).map(|to| to.now));
    assert_eq!(
        carried,
        Ok(Some("/mnt/elsewhere".to_owned())),
        "and it is the comparison that refused, not the guard refusing everything"
    );
}

#[tokio::test]
async fn an_accepted_re_point_leaves_the_restored_settings_naming_this_machine() {
    // The archive's own data root is not on this machine, so the file that
    // landed names a library that is not here until this puts it right.
    let dir = scratch("restore-repoint");
    let _ = std::fs::remove_dir_all(&dir);
    let created = std::fs::create_dir_all(&dir);
    assert!(created.is_ok(), "the scratch directory was made");

    let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
    let restoration = run(
        &rooted_at(&dir, "/mnt/library", &vault),
        &Kept::Named(KEPT.to_owned()),
        true,
        &Consent::Standing,
    )
    .await
    .map_err(|problem| problem.code);
    assert_eq!(
        restoration.map(|said| said.done.and_then(|done| done.relocated).map(|to| to.now)),
        Ok(Some("/mnt/library".to_owned()))
    );
    let written = std::fs::read_to_string(dir.join(".env")).unwrap_or_default();
    assert!(written.contains("DATA_ROOT=/mnt/library"), "{written}");
}

#[tokio::test]
async fn settings_that_landed_but_could_not_be_re_pointed_are_reported() {
    // The archive is in place and its recorded root is not this machine's, so
    // the restore is not the success its own report would otherwise claim.
    let dir = scratch("restore-unwritable");
    let _ = std::fs::remove_dir_all(&dir);
    let blocked = std::fs::create_dir_all(dir.join(".env"));
    assert!(blocked.is_ok(), "the environment file's place is taken");

    let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
    let refusal = run(
        &rooted_at(&dir, "/mnt/library", &vault),
        &Kept::Named(KEPT.to_owned()),
        true,
        &Consent::Standing,
    )
    .await
    .err()
    .map(|problem| problem.code);
    assert_eq!(refusal, Some(NOT_REPOINTED));
}
