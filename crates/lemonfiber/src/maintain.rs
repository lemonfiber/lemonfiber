//! The `backup` and `restore` subcommands.
//!
//! Both drive the pure executors in the core over the real `tar` adapter, and
//! both refuse to touch service databases while the stack is running: a capture of
//! a live `SQLite` file, or a restore over one, is the corruption a backup exists to
//! prevent. Stopping and restarting the stack around the operation for the operator
//! is a convenience these will grow; refusing a running stack is the honest floor.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lemonfiber_core::app::backup::capture;
use lemonfiber_core::app::restore::{inspect, restore};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::backup::{Retention, Scope, SCHEMA};
use lemonfiber_core::config::paths::Paths;

mod guard;
mod report;

use guard::{repoint_env, require_stopped, stamp};
use report::{next_steps, render_backup, render_preview, render_restore};

use crate::archive::Tar;

/// What stands in for a report that could not be turned into JSON — a value rather
/// than an unreachable branch, for the reason the renderer's own fallback is.
const UNSERIALISABLE: &str = r#"{"error":"this report could not be rendered as JSON"}"#;

/// How many backups of each scope are kept before the oldest are pruned.
const KEEP: usize = 5;

/// Capture the configuration to a backup archive.
pub(crate) async fn run_backup(
    ctx: Ctx,
    paths: Paths,
    service: Option<String>,
    json: bool,
) -> ExitCode {
    if let Some(code) = require_stopped(&ctx, "backup").await {
        return code;
    }

    let scope = match service {
        Some(name) => Scope::Service { name },
        None => Scope::WholeStack,
    };
    let data_root = ctx
        .settings
        .data_root
        .as_deref()
        .map(Path::to_string_lossy)
        .unwrap_or_default()
        .into_owned();

    match capture(
        &paths,
        scope,
        env!("CARGO_PKG_VERSION"),
        &stamp(),
        &data_root,
        Retention::keeping(KEEP),
        &Tar,
    )
    .await
    {
        Ok(report) => {
            render_backup(&report, json).print();
            ExitCode::SUCCESS
        }
        Err(problem) => crate::complain(problem.as_ref()),
    }
}

/// Restore the configuration from a backup archive.
pub(crate) async fn run_restore(
    ctx: Ctx,
    paths: Paths,
    archive: PathBuf,
    repoint: bool,
    json: bool,
) -> ExitCode {
    let current_root = ctx.settings.data_root.clone().unwrap_or_default();

    // Verify and list what the archive holds before anything is overwritten.
    match inspect(
        &archive,
        env!("CARGO_PKG_VERSION"),
        SCHEMA,
        &current_root,
        &Tar,
    )
    .await
    {
        Ok(preview) => render_preview(&preview, json).print(),
        Err(problem) => return crate::complain(problem.as_ref()),
    }

    if let Some(code) = require_stopped(&ctx, "restore").await {
        return code;
    }

    match restore(
        &archive,
        &paths,
        env!("CARGO_PKG_VERSION"),
        SCHEMA,
        &current_root,
        repoint,
        &Tar,
    )
    .await
    {
        Ok(report) => {
            // Complete the re-point the executor recorded: the restored `.env` still
            // names the data root the backup was taken with, which is not on this
            // machine, so it is set to this one now that the files are in place —
            // the adjustment the re-point offered.
            if let Some(code) = report
                .relocated
                .as_ref()
                .and_then(|relocation| repoint_env(&paths, relocation))
            {
                return code;
            }
            render_restore(&report, json).print();
            // A restore replaces state while the stack is down, so the wiring between
            // services and the credentials it holds are reconciled once it is back up.
            next_steps().eprint();
            ExitCode::SUCCESS
        }
        Err(problem) => crate::complain(problem.as_ref()),
    }
}

/// What the engine says about whether the stack is running.
enum Stack {
    /// At least one container is running and may be writing to its database.
    Running,
    /// The engine answered and nothing is running.
    Stopped,
    /// The engine could not be reached, so the stack cannot be confirmed stopped.
    Unknown,
}

#[cfg(test)]
mod tests {
    use crate::exit::{shown, success};
    use std::sync::Arc;

    use async_trait::async_trait;
    use lemonfiber_core::backup::{Manifest, Member, Relocation, Scope};
    use lemonfiber_core::config::Settings;
    use lemonfiber_core::platform::Environment;
    use lemonfiber_core::ports::docker::{
        Container, Engine, ExecOutput, Failure, Health, Lifecycle, LogLine, LogQuery, Stats,
    };
    use lemonfiber_core::stack::Source;
    use tokio::sync::mpsc::Receiver;

    use super::guard::{refuse, stack_from, stamp};
    use super::report::{next_steps, render_backup, render_preview, render_restore, scope_name};
    use lemonfiber_core::app::restore::Preview;

    use super::{Ctx, ExitCode, Paths, Stack};

    /// An engine that answers a listing however a test needs, and nothing else —
    /// the only capability a backup's refusal actually consults.
    struct FakeEngine(Option<Lifecycle>);

    #[async_trait]
    impl Engine for FakeEngine {
        async fn list(&self, _project: &str) -> Result<Vec<Container>, Failure> {
            let Some(lifecycle) = self.0 else {
                return Err(Failure::Unreachable {
                    reason: "no engine".to_owned(),
                });
            };
            Ok(vec![a_container(lifecycle)])
        }

        async fn exec(&self, _container: &str, _argv: &[String]) -> Result<ExecOutput, Failure> {
            Err(unused())
        }

        async fn stats(&self, _project: &str) -> Result<Receiver<(String, Stats)>, Failure> {
            Err(unused())
        }

        async fn logs(
            &self,
            _project: &str,
            _services: &[String],
            _query: LogQuery,
        ) -> Result<Receiver<LogLine>, Failure> {
            Err(unused())
        }
    }

    /// The refusal the capabilities a backup never reaches for answer with, so the
    /// fake states plainly that they are not part of this decision.
    fn unused() -> Failure {
        Failure::Unreachable {
            reason: "a backup never asks this".to_owned(),
        }
    }

    fn a_container(lifecycle: Lifecycle) -> Container {
        Container {
            id: "abc".to_owned(),
            project: "lemonfiber".to_owned(),
            service: "sonarr".to_owned(),
            lifecycle,
            health: Health::None,
            exit: None,
        }
    }

    fn ctx_with(engine: FakeEngine) -> Ctx {
        Ctx::new(
            Arc::new(lemonfiber_core::adapters::Local),
            Arc::new(engine),
            Arc::new(lemonfiber_core::adapters::System),
            Arc::new(lemonfiber_core::adapters::Disk),
            Source::Embedded(&crate::cli::STACK),
            Settings::default(),
            Environment::MacOs,
        )
    }

    fn a_manifest() -> Manifest {
        Manifest {
            schema: 1,
            product_version: "0.3.0".to_owned(),
            created_at: "1700000000".to_owned(),
            data_root: "/srv/media".to_owned(),
            scope: Scope::WholeStack,
            sensitive: true,
            members: vec![Member {
                label: "the configuration".to_owned(),
                archive_path: "config".to_owned(),
            }],
        }
    }

    /// A scratch install with something in each area, unique to this test.
    fn install(name: &str) -> Paths {
        let root =
            std::env::temp_dir().join(format!("lemonfiber-maintain-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = Paths::rooted(&root.join("config"), &root.join("data"));
        for (path, contents) in [
            (paths.env_file(), "DATA_ROOT=/srv/media\n"),
            (
                paths.service_config().join("sonarr/config.xml"),
                "<Config/>",
            ),
            (paths.stack().join("compose.yaml"), "services: {}"),
        ] {
            let _ = path.parent().map(std::fs::create_dir_all);
            let _ = std::fs::write(&path, contents);
        }
        paths
    }

    /// A context over a stack the engine says is stopped, so an operation proceeds.
    fn stopped() -> Ctx {
        ctx_with(FakeEngine(Some(Lifecycle::Exited)))
    }

    #[tokio::test]
    async fn a_backup_of_a_stopped_stack_writes_an_archive_and_then_restores_it() {
        let paths = install("round-trip");
        // Capture: the engine says nothing is running, so it goes ahead.
        let code = super::run_backup(stopped(), paths.clone(), None, false).await;
        assert_eq!(shown(code), success());

        let written = std::fs::read_dir(paths.backups())
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .next();
        let archive = written.unwrap_or_default();
        assert!(archive.exists(), "a backup was written");

        // Restore it back over the same install: the preview is shown, then the
        // files are put back and the operator is told what follows.
        let restored = super::run_restore(stopped(), paths, archive, false, false).await;
        assert_eq!(shown(restored), success());
    }

    #[tokio::test]
    async fn a_backup_scoped_to_one_service_reports_as_json_when_asked() {
        let paths = install("one-service");
        let code = super::run_backup(stopped(), paths, Some("sonarr".to_owned()), true).await;
        assert_eq!(shown(code), success());
    }

    #[tokio::test]
    async fn a_running_stack_refuses_both_operations_before_touching_anything() {
        let paths = install("running");
        let running = || ctx_with(FakeEngine(Some(Lifecycle::Running)));
        let backup = super::run_backup(running(), paths.clone(), None, false).await;
        assert_eq!(shown(backup), shown(ExitCode::FAILURE));
        // Nothing was written: the refusal comes before the capture.
        assert!(std::fs::read_dir(paths.backups()).is_err());
    }

    #[tokio::test]
    async fn an_archive_that_is_not_there_is_refused_before_the_stack_is_consulted() {
        let paths = install("no-archive");
        let missing = paths.backups().join("nothing.tar.gz");
        let code = super::run_restore(stopped(), paths, missing, false, false).await;
        assert_eq!(shown(code), shown(ExitCode::FAILURE));
    }

    #[tokio::test]
    async fn the_capabilities_a_backup_never_asks_for_answer_that_plainly() {
        // The fake stands in for a whole engine, and a backup consults only its
        // listing. Exercised so the fake cannot quietly grow a wrong answer.
        let engine = FakeEngine(None);
        assert!(engine.exec("c", &[]).await.is_err());
        assert!(engine.stats("p").await.is_err());
        assert!(engine.logs("p", &[], LogQuery::recent(10)).await.is_err());
        assert!(engine.list("p").await.is_err());
    }

    /// A context over a stopped stack that reports the given data root.
    fn stopped_at(data_root: &str) -> Ctx {
        let mut ctx = stopped();
        ctx.settings.data_root = Some(std::path::PathBuf::from(data_root));
        ctx
    }

    #[tokio::test]
    async fn a_capture_that_cannot_be_written_is_reported_rather_than_claimed() {
        let paths = install("blocked");
        // A file where the backups directory belongs: the capture cannot create it,
        // and says so rather than reporting a backup nobody has.
        let _ = paths.backups().parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(paths.backups(), "not a directory");
        let code = super::run_backup(stopped(), paths, None, false).await;
        assert_eq!(shown(code), shown(ExitCode::FAILURE));
    }

    #[tokio::test]
    async fn a_restore_over_a_running_stack_is_refused_after_the_preview() {
        let paths = install("restore-running");
        assert_eq!(
            format!(
                "{:?}",
                super::run_backup(stopped(), paths.clone(), None, false).await
            ),
            success()
        );
        let archive = one_backup(&paths);

        // The preview still runs — it overwrites nothing — but the restore itself
        // is refused while something may be writing.
        let running = ctx_with(FakeEngine(Some(Lifecycle::Running)));
        let code = super::run_restore(running, paths, archive, false, false).await;
        assert_eq!(shown(code), shown(ExitCode::FAILURE));
    }

    #[tokio::test]
    async fn a_restore_onto_a_different_data_root_repoints_the_env_it_put_back() {
        let paths = install("repoint");
        assert_eq!(
            format!(
                "{:?}",
                super::run_backup(stopped_at("/old/media"), paths.clone(), None, false).await
            ),
            success()
        );
        let archive = one_backup(&paths);

        // Restored on a machine whose data root is elsewhere, with the re-point the
        // preview offered: the archive's own env named the old root, and the file
        // that lands has to name this machine's.
        let code = super::run_restore(
            stopped_at("/new/media"),
            paths.clone(),
            archive,
            true,
            false,
        )
        .await;
        assert_eq!(shown(code), success());
        let env = std::fs::read_to_string(paths.env_file()).unwrap_or_default();
        assert!(
            env.contains("/new/media"),
            "the data root was re-pointed: {env}"
        );
    }

    #[tokio::test]
    async fn an_archive_that_verifies_but_will_not_unpack_is_reported() {
        // Its manifest reads, so the preview passes; the bytes beside it name an
        // area this build does not know, so unpacking refuses. The manifest and the
        // members are not the same evidence, which is the whole reason both are
        // checked.
        let paths = install("bad-member");
        let archive = paths.backups().join("forged.tar.gz");
        let _ = archive.parent().map(std::fs::create_dir_all);
        let manifest = Manifest {
            data_root: String::new(),
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
            ..a_manifest()
        };
        let _ = write_forged(
            &archive,
            &manifest,
            &[("secrets/leak", tar::EntryType::Regular)],
        );

        let code = super::run_restore(stopped(), paths, archive, false, false).await;
        assert_eq!(shown(code), shown(ExitCode::FAILURE));
    }

    /// The single archive a capture left in the backups directory.
    fn one_backup(paths: &Paths) -> std::path::PathBuf {
        std::fs::read_dir(paths.backups())
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .next()
            .unwrap_or_default()
    }

    /// An archive whose manifest is sound but whose members are whatever a test
    /// needs — the manifest and the bytes beside it are not the same evidence, which
    /// is the whole reason a restore checks both.
    fn write_forged(
        dest: &std::path::Path,
        manifest: &Manifest,
        members: &[(&str, tar::EntryType)],
    ) -> Option<()> {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let file = std::fs::File::create(dest).ok()?;
        let mut builder = tar::Builder::new(GzEncoder::new(file, Compression::default()));
        let json = serde_json::to_vec(manifest).unwrap_or_default();
        let mut header = tar::Header::new_gnu();
        header.set_size(json.len() as u64);
        header.set_mode(0o600);
        header.set_cksum();
        let _ = builder.append_data(&mut header, "manifest.json", json.as_slice());
        for (path, kind) in members {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(*kind);
            header.set_size(0);
            header.set_mode(0o600);
            header.set_cksum();
            let _ = builder.append_data(&mut header, path, std::io::empty());
        }
        builder.into_inner().ok()?.finish().ok().map(|_| ())
    }

    #[test]
    fn a_repoint_that_cannot_be_written_is_reported_rather_than_passed_over() {
        // The restored files are in place but the environment file cannot be
        // written, so the data root would silently still name the old machine's.
        // Reported instead — a restore that half-landed is worse than one refused.
        let paths = install("repoint-blocked");
        let _ = std::fs::remove_file(paths.env_file());
        // A directory where the file belongs: writing it cannot succeed.
        let _ = std::fs::create_dir_all(paths.env_file());
        let code = super::repoint_env(
            &paths,
            &Relocation {
                was: "/old".to_owned(),
                now: "/new".to_owned(),
            },
        );
        assert!(code.is_some());
    }

    #[tokio::test]
    async fn a_restore_that_lands_but_cannot_be_repointed_says_so() {
        // The files come back, but the environment file that landed is a directory,
        // so the data root cannot be set to this machine's. Reported rather than
        // passed over: an install still naming another machine's root is the
        // half-landed restore the re-point exists to finish.
        let paths = install("repoint-fails");
        let archive = paths.backups().join("odd.tar.gz");
        let _ = archive.parent().map(std::fs::create_dir_all);
        let manifest = Manifest {
            data_root: "/old/media".to_owned(),
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
            ..a_manifest()
        };
        let _ = write_forged(
            &archive,
            &manifest,
            &[("config/.env", tar::EntryType::Directory)],
        );

        let code = super::run_restore(stopped_at("/new/media"), paths, archive, true, false).await;
        // The code is whichever the problem's own severity earns; what matters is
        // that it is not a success.
        assert_ne!(shown(code), success());
    }

    #[test]
    fn a_scope_reads_as_what_it_covers() {
        assert_eq!(scope_name(&Scope::WholeStack), "the whole stack");
        assert_eq!(
            scope_name(&Scope::Service {
                name: "sonarr".to_owned()
            }),
            "service sonarr"
        );
    }

    #[test]
    fn a_stamp_is_a_sortable_number_of_seconds() {
        let stamped = stamp();
        assert!(!stamped.is_empty());
        assert!(stamped.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn a_listing_says_whether_anything_could_be_writing() {
        // Anything running means a database may be mid-write.
        assert!(matches!(
            stack_from(Ok(vec![a_container(Lifecycle::Running)])),
            Stack::Running
        ));
        // Everything stopped, and nothing at all, both mean nothing is writing.
        assert!(matches!(
            stack_from(Ok(vec![a_container(Lifecycle::Exited)])),
            Stack::Stopped
        ));
        assert!(matches!(stack_from(Ok(Vec::new())), Stack::Stopped));
        // An engine that will not answer cannot prove anything, so it is unknown.
        assert!(matches!(
            stack_from(Err(Failure::Unreachable {
                reason: "down".to_owned()
            })),
            Stack::Unknown
        ));
    }

    #[test]
    fn the_refusal_fails_closed_on_anything_but_a_confirmed_stop() {
        // Confirmed stopped is the only state that proceeds.
        assert!(refuse(&Stack::Stopped, "backup").is_none());
        let running = refuse(&Stack::Running, "backup").unwrap_or_default().text();
        assert!(running.contains("must not happen while the"));
        assert!(running.contains("lemonfiber down"));
        // An engine that will not answer is refused as firmly as a running stack:
        // it cannot prove nothing is writing, and a guess is the corruption a
        // backup exists to prevent.
        let unknown = refuse(&Stack::Unknown, "restore")
            .unwrap_or_default()
            .text();
        assert!(unknown.contains("could not reach the container engine"));
        assert!(unknown.contains("will not risk a restore"));
    }

    #[tokio::test]
    async fn a_running_or_unreachable_stack_stops_a_backup_before_it_starts() {
        use super::require_stopped;
        assert!(
            require_stopped(&ctx_with(FakeEngine(Some(Lifecycle::Exited))), "backup")
                .await
                .is_none()
        );
        assert!(
            require_stopped(&ctx_with(FakeEngine(Some(Lifecycle::Running))), "backup")
                .await
                .is_some()
        );
        // The engine that answers nothing at all.
        assert!(require_stopped(&ctx_with(FakeEngine(None)), "restore")
            .await
            .is_some());
    }

    #[test]
    fn a_backup_report_names_what_it_took_and_warns_what_is_in_it() {
        use lemonfiber_core::app::backup::Report as BackupReport;
        let report = BackupReport {
            scope: Scope::WholeStack,
            path: std::path::PathBuf::from("/backups/b.tar.gz"),
            sensitive: true,
            pruned: vec!["old.tar.gz".to_owned()],
        };
        let text = render_backup(&report, false).text();
        assert!(text.contains("Backed up the whole stack to /backups/b.tar.gz"));
        assert!(text.contains("contains credentials"));
        assert!(text.contains("Pruned 1 older backup(s)."));

        // Nothing sensitive and nothing pruned says neither.
        let plain = BackupReport {
            sensitive: false,
            pruned: Vec::new(),
            ..report.clone()
        };
        let quiet = render_backup(&plain, false).text();
        assert!(!quiet.contains("credentials"));
        assert!(!quiet.contains("Pruned"));

        // As JSON it is one line a script can parse.
        assert!(render_backup(&report, true).text().contains("\"scope\""));
    }

    #[test]
    fn a_preview_lists_what_the_archive_holds_before_anything_is_overwritten() {
        let preview = Preview {
            manifest: a_manifest(),
            downgrade: true,
            relocation: Some(Relocation {
                was: "/old".to_owned(),
                now: "/new".to_owned(),
            }),
        };
        let text = render_preview(&preview, false).text();
        assert!(text.contains("holds the whole stack, taken by lemonfiber 0.3.0"));
        assert!(text.contains("- the configuration"));
        assert!(text.contains("older major version"));
        assert!(text.contains("--repoint"));

        // Same version, same data root: neither caveat is raised.
        let matching = Preview {
            manifest: a_manifest(),
            downgrade: false,
            relocation: None,
        };
        let quiet = render_preview(&matching, false).text();
        assert!(!quiet.contains("older major version"));
        assert!(!quiet.contains("--repoint"));

        assert!(render_preview(&preview, true)
            .text()
            .contains(r#""kind":"restore-preview""#));
    }

    #[test]
    fn a_restore_report_says_what_came_back_and_where_it_was_pointed() {
        use lemonfiber_core::app::restore::Report as RestoreReport;
        let report = RestoreReport {
            scope: Scope::WholeStack,
            from_version: "0.3.0".to_owned(),
            relocated: Some(Relocation {
                was: "/old".to_owned(),
                now: "/new".to_owned(),
            }),
        };
        let text = render_restore(&report, false).text();
        assert!(text.contains("Restored the whole stack"));
        assert!(text.contains("Re-pointed the data root from /old to /new."));

        let stayed = RestoreReport {
            relocated: None,
            ..report.clone()
        };
        assert!(!render_restore(&stayed, false).text().contains("Re-pointed"));
        assert!(render_restore(&report, true)
            .text()
            .contains(r#""kind":"restore""#));
    }

    #[test]
    fn a_restore_leaves_the_operator_the_two_steps_that_follow_it() {
        let text = next_steps().text();
        assert!(text.contains("lemonfiber up <form> && lemonfiber seed"));
        assert!(text.contains("doctor --only credentials"));
    }
}
