//! The survey that reads what is already on a machine, dispatched as the surfaces reach it.
//!
//! Beside the crate rather than inside it because the dispatcher is compiled twice —
//! once with the crate's own tests and once without — so an arm exercised only in-crate
//! has its coverage counted from the copy that never ran.
//!
//! What is asserted here is the half the pure survey cannot: that the command routes,
//! and that each way of failing to look reports it could not look rather than reporting
//! an empty machine. The difference decides whether it is safe to stand a stack up over
//! somebody's library, so every seam that can refuse is driven to refuse.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::stack::project;
use lemonfiber_core::app::{dispatch, Command, Ctx, MigrateAction, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::migration::mode::Mode;
use lemonfiber_core::model::MigrationReport;
use lemonfiber_core::model::{AdoptReport, BesideReport, ReplaceReport};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::filesystem::{FsKind, StorageFacts};
use lemonfiber_core::ports::http::Method;
use lemonfiber_core::ports::Runner;
use lemonfiber_core::reconfigure::Stance;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{refused, spoke, Recording, Reporting, Scripted, SeedFs};

/// A machine whose engine answers with the given containers and images.
fn ctx(engine: Reporting, images: Arc<Pulled>) -> Ctx {
    over(engine, images, Source::External(project()))
}

/// The same machine, reading its stack from somewhere named.
fn over(engine: Reporting, images: Arc<Pulled>, stack: Source) -> Ctx {
    driven(engine, images, stack, Arc::new(Scripted(Ok(spoke("")))))
}

/// A filesystem whose paths under `point` all belong to that one mount.
fn facts(point: &str) -> StorageFacts {
    StorageFacts {
        point: PathBuf::from(point),
        kind: FsKind::Linking("apfs".to_owned()),
        removable: false,
        available: 100,
        total: 1_000,
    }
}

/// The same machine, reading its filesystem through a given fake.
fn over_files(engine: Reporting, images: Arc<Pulled>, files: Arc<SeedFs>) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(engine),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: files,
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings {
            project: "lemonfiber".to_owned(),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_images(images)
}

/// The same machine again, with the programs it runs answered by a given runner.
///
/// Apart from the others because a claim about what a survey *did not* run cannot be
/// made from what came back: it has to be asked of the thing that would have run it.
fn driven(engine: Reporting, images: Arc<Pulled>, stack: Source, runner: Arc<dyn Runner>) -> Ctx {
    Ctx::new(
        runner,
        Arc::new(engine),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: Arc::new(SeedFs::keyed(None, None).with_facts(StorageFacts {
                point: PathBuf::from("/srv/media"),
                kind: FsKind::Linking("apfs".to_owned()),
                removable: false,
                available: 100,
                total: 1_000,
            })),
            ..lemonfiber_adapters::live()
        },
        stack,
        Settings {
            project: "lemonfiber".to_owned(),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_images(images)
}

/// What the survey answered, or nothing where the command refused.
async fn surveyed(ctx: &Ctx) -> Option<MigrationReport> {
    match dispatch(Command::Migrate(MigrateAction::Survey), ctx).await {
        Ok(Outcome::Migration(report)) => Some(report),
        _ => None,
    }
}

/// The same engine, with the given host paths mounted into its containers.
fn mounting(engine: Reporting, paths: &[&str]) -> Reporting {
    engine.mounting(&paths.iter().map(PathBuf::from).collect::<Vec<_>>())
}

/// An engine holding one service, under a project that is not lemonfiber's.
fn somebody_elses() -> Reporting {
    Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
        .belonging_to("media")
        .publishing(&[("sonarr", "127.0.0.1", 8989)])
}

#[tokio::test]
async fn the_survey_reaches_the_command_and_names_the_project_it_found() {
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let found = surveyed(&ctx(somebody_elses(), images)).await;
    let named = found
        .as_ref()
        .and_then(|report| report.standing.first())
        .map(|standing| standing.project.clone());
    assert_eq!(named, Some("media".to_owned()), "{found:?}");
}

#[tokio::test]
async fn a_port_we_want_that_the_existing_stack_holds_is_named_before_anything_is_proposed() {
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let found = surveyed(&ctx(somebody_elses(), images)).await;
    let held = found
        .as_ref()
        .and_then(|report| report.conflicts.first())
        .map(|clash| clash.held_by.clone());
    assert_eq!(held, Some("media/sonarr".to_owned()), "{found:?}");
}

#[tokio::test]
async fn an_engine_that_will_not_list_reports_it_could_not_look() {
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let found = surveyed(&ctx(Reporting::absent(), images)).await;
    let read = found.as_ref().map(|report| report.read);
    assert_eq!(read, Some(false), "{found:?}");
}

#[tokio::test]
async fn an_engine_that_will_not_say_what_it_pulled_reports_it_could_not_look() {
    let refused = Pulled::unreachable("no daemon here");
    let found = surveyed(&ctx(somebody_elses(), refused)).await;
    let read = found.as_ref().map(|report| report.read);
    assert_eq!(read, Some(false), "{found:?}");
}

#[tokio::test]
async fn a_stack_whose_manifest_cannot_be_read_reports_it_could_not_look() {
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let nowhere = Source::External(Path::new("/nowhere-at-all"));
    let found = surveyed(&over(somebody_elses(), images, nowhere)).await;
    let read = found.as_ref().map(|report| report.read);
    assert_eq!(read, Some(false), "{found:?}");
}

#[tokio::test]
async fn a_machine_with_nothing_else_on_it_says_it_looked() {
    let found = surveyed(&ctx(Reporting::absent(), Pulled::holding(Vec::new()))).await;
    let read = found.as_ref().map(|report| report.read);
    assert_eq!(read, Some(true), "{found:?}");
    let standing = found.as_ref().map(|report| report.standing.len());
    assert_eq!(standing, Some(0), "{found:?}");
}

/// A survey is a read. Nothing it does may reach the operator's running stack, because
/// a migration they abandon halfway has to leave them exactly what they had.
#[tokio::test]
async fn a_survey_runs_no_program_at_all() {
    let watching = Arc::new(Recording::answering(Ok(spoke(""))));
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let ctx = driven(
        somebody_elses(),
        images,
        Source::External(project()),
        Arc::clone(&watching) as Arc<dyn Runner>,
    );

    let found = surveyed(&ctx).await;
    assert!(found.is_some(), "the survey answered");

    // Everything it was handed, not one word at a time: the invocation a narrower
    // question forgot to ask about is the one that would have stopped their stack.
    let ran = watching.seen();
    assert!(ran.is_empty(), "a survey ran a program: {ran:?}");
}

/// A layout whose data sits on two filesystems cannot hardlink between them, and the
/// survey has to say so from what the engine actually reported being mounted.
#[tokio::test]
async fn a_layout_across_two_filesystems_is_reported_with_its_cost_and_a_remedy() {
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let engine = mounting(somebody_elses(), &["/srv/media", "/mnt/downloads"]);

    let split = SeedFs::keyed(None, None)
        .with_facts(facts("/srv"))
        .with_facts_under("/mnt", facts("/mnt"));

    let ctx = over_files(engine, images, Arc::new(split));
    let found = surveyed(&ctx).await.and_then(|report| report.linking);
    assert_eq!(
        found.as_ref().map(|read| (read.links, read.forced)),
        Some((false, false)),
        "two filesystems is a finding the operator decides about: {found:?}"
    );
    let said = found
        .as_ref()
        .map(|read| read.because.clone())
        .unwrap_or_default();
    assert!(said.contains("/mnt") && said.contains("/srv"), "{said}");
    let remedy = found.map(|read| read.remedy).unwrap_or_default();
    assert!(!remedy.is_empty(), "a remedy is offered");
}

/// One filesystem is the ordinary case and is not worth telling anybody about.
#[tokio::test]
async fn a_layout_on_one_filesystem_is_not_reported_at_all() {
    let images = Pulled::holding(vec![Pulled::image("sonarr", 400, &["media"])]);
    let engine = mounting(somebody_elses(), &["/srv/media", "/srv/downloads"]);
    let one = SeedFs::keyed(None, None).with_facts(facts("/srv"));

    let ctx = over_files(engine, images, Arc::new(one));
    let found = surveyed(&ctx).await.and_then(|report| report.linking);
    assert!(found.is_none(), "{found:?}");
}

/// A scratch environment file adopting can record its answer in.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-adopt-{}-{name}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = std::fs::remove_file(&env);
    env
}

/// A machine whose existing project runs a service lemonfiber knows, at a given version.
fn theirs(version: &str, env: Option<PathBuf>) -> Ctx {
    let images = Pulled::holding(vec![Pulled::image(
        &format!("lscr.io/linuxserver/sonarr:{version}"),
        400,
        &["media"],
    )]);
    let mut ctx = over(somebody_elses(), images, Source::External(project()));
    ctx.settings.env_file = env;
    ctx
}

/// What adopting answered, or nothing where it refused to answer at all.
async fn adopting(ctx: &Ctx, confirmed: bool) -> Option<AdoptReport> {
    match dispatch(
        Command::Migrate(MigrateAction::Act {
            mode: Mode::Adopt,
            confirmed,
        }),
        ctx,
    )
    .await
    {
        Ok(Outcome::Adoption(report)) => Some(report),
        _ => None,
    }
}

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

/// What standing beside answered, or nothing where it refused to answer at all.
async fn standing(ctx: &Ctx, confirmed: bool) -> Option<BesideReport> {
    match dispatch(
        Command::Migrate(MigrateAction::Act {
            mode: Mode::Beside,
            confirmed,
        }),
        ctx,
    )
    .await
    {
        Ok(Outcome::Beside(report)) => Some(report),
        _ => None,
    }
}

#[tokio::test]
async fn standing_beside_unconfirmed_says_where_it_would_listen_and_writes_nothing() {
    let env = scratch("beside-rehearsed");
    let found = standing(&theirs("4.0.15", Some(env.clone())), false).await;
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
    ctx.settings.env_file = Some(env.clone());

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
    let blocked = std::env::temp_dir().join(format!("lemonfiber-beside-{}", std::process::id()));
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

/// What standing in place of it answered.
async fn replacing(ctx: &Ctx, confirmed: bool) -> Option<ReplaceReport> {
    match dispatch(
        Command::Migrate(MigrateAction::Act {
            mode: Mode::Replace,
            confirmed,
        }),
        ctx,
    )
    .await
    {
        Ok(Outcome::Replacement(report)) => Some(report),
        _ => None,
    }
}

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

/// Their sonarr, on a port of its own so the two stacks can be told apart.
fn both_stacks() -> (Reporting, Arc<Pulled>) {
    let engine = Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
        .belonging_to("media")
        .publishing(&[("sonarr", "127.0.0.1", 18989)])
        .mounting(&[PathBuf::from("/their/sonarr")]);
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.15",
        400,
        &["media"],
    )]);
    (engine, images)
}

/// A transport answering as two stacks at once, told apart by the port asked.
fn two_stacks() -> Arc<lemonfiber_fixtures::http::Fake> {
    use lemonfiber_fixtures::http::{Answer, Fake};
    Fake::by_route(vec![
        // Theirs: one series, following a profile it numbered 1.
        (
            Method::Get,
            "18989/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":1,"name":"HD"}]"#),
        ),
        (
            Method::Get,
            "18989/api/v3/series",
            Answer::reply(
                200,
                r#"[{"id":5,"title":"Taskmaster","qualityProfileId":1,"rootFolderPath":"/data/tv"}]"#,
            ),
        ),
        (
            Method::Get,
            "18989/api/v3/indexer",
            Answer::reply(200, "[]"),
        ),
        // Ours: the same profile, numbered differently, and nothing followed yet.
        (
            Method::Get,
            ":8989/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":7,"name":"HD"}]"#),
        ),
        (Method::Get, ":8989/api/v3/series", Answer::reply(200, "[]")),
        (
            Method::Get,
            ":8989/api/v3/indexer",
            Answer::reply(200, "[]"),
        ),
        (
            Method::Post,
            ":8989/api/v3/series",
            Answer::reply(201, "{}"),
        ),
    ])
}

/// A machine holding both stacks, reached through the given transport.
fn importing(http: Arc<lemonfiber_fixtures::http::Fake>) -> Ctx {
    let (engine, images) = both_stacks();
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(engine),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: Arc::new(SeedFs::keyed(
                Some("<Config><ApiKey>the-key</ApiKey></Config>"),
                None,
            )),
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings {
            project: "lemonfiber".to_owned(),
            stack_dir: Some(PathBuf::from("/srv/lemonfiber")),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_images(images)
    .with_http(http)
}

/// What carrying answered.
async fn carrying(ctx: &Ctx, confirmed: bool) -> Option<lemonfiber_core::model::ImportReport> {
    match dispatch(
        Command::Migrate(MigrateAction::Act {
            mode: Mode::Import,
            confirmed,
        }),
        ctx,
    )
    .await
    {
        Ok(Outcome::Import(report)) => Some(report),
        _ => None,
    }
}

#[tokio::test]
async fn carrying_unconfirmed_names_what_would_travel_and_writes_nothing() {
    let http = two_stacks();
    let found = carrying(&importing(Arc::clone(&http)), false).await;
    let named: Vec<String> = found
        .map(|read| read.would_carry.into_iter().map(|one| one.name).collect())
        .unwrap_or_default();
    assert_eq!(named, vec!["Taskmaster".to_owned()], "what would travel");

    let posted = http
        .requests()
        .into_iter()
        .any(|request| request.method == Method::Post);
    assert!(!posted, "a rehearsal wrote to a service");
}

/// The assertion the whole design turns on: two stacks number their own profiles, so a
/// record carried with the old number would follow whatever happened to be first here.
#[tokio::test]
async fn a_carried_record_follows_this_stacks_own_profile_not_the_number_it_had() {
    let http = two_stacks();
    let found = carrying(&importing(Arc::clone(&http)), true).await;
    let carried: Vec<String> = found
        .map(|read| read.carried.into_iter().map(|one| one.name).collect())
        .unwrap_or_default();
    assert_eq!(carried, vec!["Taskmaster".to_owned()], "what travelled");

    let body = http
        .requests()
        .into_iter()
        .find(|request| request.method == Method::Post)
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(body.contains("\"qualityProfileId\":7"), "remapped: {body}");
    assert!(
        !body.contains("\"qualityProfileId\":1"),
        "not theirs: {body}"
    );
    assert!(
        !body.contains("\"id\":5"),
        "its id there is not its id here: {body}"
    );
}

/// A machine holding both stacks, with the pieces a caller wants to vary.
fn importing_over(
    engine: Reporting,
    stack: Source,
    http: Arc<lemonfiber_fixtures::http::Fake>,
) -> Ctx {
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.15",
        400,
        &["media"],
    )]);
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(engine),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: Arc::new(SeedFs::keyed(
                Some("<Config><ApiKey>the-key</ApiKey></Config>"),
                None,
            )),
            ..lemonfiber_adapters::live()
        },
        stack,
        Settings {
            project: "lemonfiber".to_owned(),
            stack_dir: Some(PathBuf::from("/srv/lemonfiber")),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_images(images)
    .with_http(http)
}

/// A stack that could not be read is one the survey has already refused, so nothing is
/// carried and nothing is claimed.
#[tokio::test]
async fn carrying_out_of_a_machine_that_could_not_be_read_carries_nothing() {
    let (engine, _) = both_stacks();
    let nowhere = Source::External(Path::new("/nowhere-at-all"));
    let ctx = importing_over(engine, nowhere, two_stacks());
    let found = carrying(&ctx, true).await;
    let refused = found.and_then(|read| read.refusal);
    assert!(refused.is_some(), "refused rather than silently empty");
}

/// A service publishing nothing is one there is no way to reach a copy of.
#[tokio::test]
async fn a_service_with_no_way_in_is_named_rather_than_passed_over() {
    let unreachable = Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
        .belonging_to("media")
        .mounting(&[PathBuf::from("/their/sonarr")]);
    let ctx = importing_over(unreachable, Source::External(project()), two_stacks());
    let found = carrying(&ctx, false).await;
    let named = found.map_or_else(Vec::new, |read| {
        read.not_carried.into_iter().map(|one| one.what).collect()
    });
    assert_eq!(
        named,
        vec!["sonarr".to_owned()],
        "named rather than dropped"
    );
}

/// A service lemonfiber does not run holds nothing this knows how to carry.
#[tokio::test]
async fn a_service_we_do_not_run_holds_nothing_to_carry() {
    let engine = Reporting::holding(&["sonarr", "ombi"], Lifecycle::Running, Health::Healthy)
        .belonging_to("media")
        .publishing(&[("sonarr", "127.0.0.1", 18989)])
        .mounting(&[PathBuf::from("/their/sonarr")]);
    let ctx = importing_over(engine, Source::External(project()), two_stacks());
    let found = carrying(&ctx, false).await;
    let named: Vec<String> = found
        .map(|read| read.would_carry.into_iter().map(|one| one.name).collect())
        .unwrap_or_default();
    assert_eq!(named, vec!["Taskmaster".to_owned()], "only what we run");
}

/// A service that will not take a record says so, rather than the import claiming it.
#[tokio::test]
async fn a_record_the_service_refuses_is_reported_rather_than_counted() {
    use lemonfiber_fixtures::http::{Answer, Fake};
    let refusing = Fake::by_route(vec![
        (
            Method::Get,
            "18989/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":1,"name":"HD"}]"#),
        ),
        (
            Method::Get,
            "18989/api/v3/series",
            Answer::reply(
                200,
                r#"[{"id":5,"title":"Taskmaster","qualityProfileId":1}]"#,
            ),
        ),
        (
            Method::Get,
            "18989/api/v3/indexer",
            Answer::reply(200, "[]"),
        ),
        (
            Method::Get,
            ":8989/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":7,"name":"HD"}]"#),
        ),
        (Method::Get, ":8989/api/v3/series", Answer::reply(200, "[]")),
        (
            Method::Get,
            ":8989/api/v3/indexer",
            Answer::reply(200, "[]"),
        ),
        (
            Method::Post,
            ":8989/api/v3/series",
            Answer::reply(500, "no"),
        ),
    ]);
    let (engine, _) = both_stacks();
    let ctx = importing_over(engine, Source::External(project()), refusing);

    let found = carrying(&ctx, true).await;
    let carried = found
        .as_ref()
        .map(|read| read.carried.len())
        .unwrap_or_default();
    assert_eq!(carried, 0, "nothing was counted as carried");
    let named = found.map_or_else(Vec::new, |read| {
        read.not_carried.into_iter().map(|one| one.what).collect()
    });
    assert_eq!(named, vec!["Taskmaster".to_owned()], "named as not carried");
}

/// Neither copy answering is a service nothing was carried out of.
#[tokio::test]
async fn a_service_neither_copy_answers_for_is_named() {
    let (engine, _) = both_stacks();
    let silent = lemonfiber_fixtures::http::Fake::silent();
    let ctx = importing_over(engine, Source::External(project()), silent);
    let found = carrying(&ctx, false).await;
    let named = found.map_or_else(Vec::new, |read| {
        read.not_carried.into_iter().map(|one| one.what).collect()
    });
    assert_eq!(named, vec!["sonarr".to_owned()], "named rather than silent");
}

/// A machine holding both stacks, with a filesystem the caller chooses.
fn importing_with(files: Arc<SeedFs>, http: Arc<lemonfiber_fixtures::http::Fake>) -> Ctx {
    let (engine, images) = both_stacks();
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(engine),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: files,
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings {
            project: "lemonfiber".to_owned(),
            stack_dir: Some(PathBuf::from("/srv/lemonfiber")),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_images(images)
    .with_http(http)
}

/// A service that has written no key yet is one neither copy can be opened for.
#[tokio::test]
async fn a_service_whose_key_cannot_be_read_is_named_rather_than_carried_from() {
    let keyless = Arc::new(SeedFs::keyed(None, None));
    let ctx = importing_with(keyless, two_stacks());
    let found = carrying(&ctx, false).await;
    let said = found.map_or_else(String::new, |read| {
        read.not_carried
            .first()
            .map(|one| one.because.clone())
            .unwrap_or_default()
    });
    assert!(said.contains("could not be reached"), "{said}");
}

/// Profiles that read and records that do not is a service half-answering, and the
/// import says which half.
#[tokio::test]
async fn a_service_whose_records_will_not_read_is_named_for_that() {
    use lemonfiber_fixtures::http::{Answer, Fake};
    let partial = Fake::by_route(vec![
        (
            Method::Get,
            "/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":7,"name":"HD"}]"#),
        ),
        (Method::Get, "/api/v3/indexer", Answer::reply(500, "no")),
    ]);
    let ctx = importing_with(
        Arc::new(SeedFs::keyed(
            Some("<Config><ApiKey>the-key</ApiKey></Config>"),
            None,
        )),
        partial,
    );
    let found = carrying(&ctx, false).await;
    let said = found.map_or_else(String::new, |read| {
        read.not_carried
            .first()
            .map(|one| one.because.clone())
            .unwrap_or_default()
    });
    assert!(said.contains("could not be read"), "{said}");
}

/// A machine that could not be looked at is one there is nothing to carry out of, and
/// saying so is different from saying the two stacks agree.
#[tokio::test]
async fn carrying_from_a_machine_that_could_not_be_looked_at_says_so() {
    let refused = Pulled::unreachable("no daemon here");
    let ctx = over(somebody_elses(), refused, Source::External(project()));
    let found = carrying(&ctx, true).await;
    let said = found.and_then(|read| read.refusal).unwrap_or_default();
    assert!(said.contains("could not be read"), "{said}");
}

/// Having looked and found no stack of ours is a different answer from not having
/// looked, and an import says which.
#[tokio::test]
async fn carrying_from_a_machine_holding_nothing_of_ours_says_there_is_no_setup() {
    let images = Pulled::holding(vec![Pulled::image("a-database:17", 400, &["shop"])]);
    let engine =
        Reporting::holding(&["postgres"], Lifecycle::Running, Health::Healthy).belonging_to("shop");
    let ctx = over(engine, images, Source::External(project()));
    let said = carrying(&ctx, true)
        .await
        .and_then(|read| read.refusal)
        .unwrap_or_default();
    assert!(said.contains("no single setup here"), "{said}");
}

/// Two stacks already holding the same records is a state of its own: nothing was
/// carried because there was nothing to carry, which is not the same as an import that
/// carried nothing because something went wrong.
#[tokio::test]
async fn two_stacks_that_already_agree_come_to_nothing_changing() {
    use lemonfiber_fixtures::http::{Answer, Fake};
    let held = r#"[{"id":5,"title":"Taskmaster","qualityProfileId":1}]"#;
    let agreeing = Fake::by_route(vec![
        (
            Method::Get,
            "18989/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":1,"name":"HD"}]"#),
        ),
        (Method::Get, "18989/api/v3/series", Answer::reply(200, held)),
        (
            Method::Get,
            "18989/api/v3/indexer",
            Answer::reply(200, "[]"),
        ),
        (
            Method::Get,
            ":8989/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":7,"name":"HD"}]"#),
        ),
        (Method::Get, ":8989/api/v3/series", Answer::reply(200, held)),
        (
            Method::Get,
            ":8989/api/v3/indexer",
            Answer::reply(200, "[]"),
        ),
    ]);
    let (engine, _) = both_stacks();
    let ctx = importing_over(engine, Source::External(project()), agreeing);

    let found = carrying(&ctx, true).await;
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Unchanged),
        "{found:?}"
    );
}

/// An engine holding one service, stopped, under a project that is not lemonfiber's.
///
/// The same setup [`somebody_elses`] describes, quiesced. A capture of a service
/// database is only safe once nothing is writing to it, so the machine a successful
/// adoption is driven over is one whose existing setup is down.
fn somebody_elses_stopped() -> Reporting {
    Reporting::holding(&["sonarr"], Lifecycle::Exited, Health::None)
        .belonging_to("media")
        .publishing(&[("sonarr", "127.0.0.1", 8989)])
}

/// Somewhere for the capture an adoption takes first, remembering what it was handed.
///
/// It records the sources rather than the destination, because what is being proved
/// here is *which tree* was captured — and a fake that only remembered that it wrote
/// would answer a question nobody is asking.
#[derive(Default)]
struct Captured(std::sync::Mutex<Vec<PathBuf>>);

impl Captured {
    /// Every source a capture was asked to read, in the order it was asked.
    fn sources(&self) -> Vec<PathBuf> {
        self.0.lock().map(|seen| seen.clone()).unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl lemonfiber_core::archive::Archive for Captured {
    async fn space(
        &self,
        _dir: &Path,
        _items: &[lemonfiber_core::backup::Item],
    ) -> Result<lemonfiber_core::archive::Space, lemonfiber_core::archive::Fault> {
        Ok(lemonfiber_core::archive::Space {
            needed: 0,
            available: 1 << 30,
        })
    }
    async fn write(
        &self,
        _dest: &Path,
        _manifest: &lemonfiber_core::backup::Manifest,
        items: &[lemonfiber_core::backup::Item],
    ) -> Result<(), lemonfiber_core::archive::Fault> {
        if let Ok(mut seen) = self.0.lock() {
            seen.extend(items.iter().map(|item| item.source.clone()));
        }
        Ok(())
    }
    async fn write_files(
        &self,
        _dest: &Path,
        _files: &[(String, String)],
    ) -> Result<(), lemonfiber_core::archive::Fault> {
        Err(lemonfiber_core::archive::Fault::new(
            "an adoption writes no bundle",
        ))
    }
    async fn existing(
        &self,
        _dir: &Path,
    ) -> Result<Vec<lemonfiber_core::backup::Existing>, lemonfiber_core::archive::Fault> {
        Ok(Vec::new())
    }
    async fn remove(
        &self,
        _dir: &Path,
        _name: &str,
    ) -> Result<(), lemonfiber_core::archive::Fault> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl lemonfiber_core::archive::Reader for Captured {
    async fn read_manifest(
        &self,
        _src: &Path,
    ) -> Result<lemonfiber_core::backup::Manifest, lemonfiber_core::archive::Fault> {
        Err(lemonfiber_core::archive::Fault::new(
            "an adoption never reads an archive back",
        ))
    }
    async fn extract(
        &self,
        _src: &Path,
        _targets: &[(String, PathBuf)],
    ) -> Result<(), lemonfiber_core::archive::Fault> {
        Err(lemonfiber_core::archive::Fault::new(
            "an adoption never reads an archive back",
        ))
    }
}

/// The same machine, with somewhere to keep what an adoption captures first.
fn keeping(ctx: Ctx, vault: &Arc<Captured>) -> Ctx {
    ctx.keeping(lemonfiber_core::archive::Archiving {
        paths: lemonfiber_core::config::paths::Paths::rooted(Path::new("/cfg"), Path::new("/data")),
        vault: Arc::clone(vault) as Arc<dyn lemonfiber_core::archive::Vault>,
    })
}

/// The capture an adoption takes first covers the setup being taken over, at the host
/// paths the survey reported — not lemonfiber's own layout, which holds nothing worth
/// protecting until the takeover has happened.
#[tokio::test]
async fn adopting_captures_the_existing_setups_own_paths_before_it_writes_anything() {
    let env = scratch("captured");
    let engine = mounting(somebody_elses_stopped(), &["/srv/their-media"]);
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.0",
        400,
        &["media"],
    )]);
    let mut ctx = over(engine, images, Source::External(project()));
    ctx.settings.env_file = Some(env);
    let vault = Arc::new(Captured::default());

    let found = adopting(&keeping(ctx, &vault), true).await;
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Applied),
        "{found:?}"
    );
    assert_eq!(
        vault.sources(),
        vec![PathBuf::from("/srv/their-media")],
        "the capture covered lemonfiber's own tree rather than the one being taken over"
    );
}

/// A setup still running cannot be captured, so it cannot be adopted either.
///
/// The capture copies that setup's own service databases, and copying a live one is
/// the corruption a backup exists to prevent. The refusal is what proves the project
/// being asked about is the existing setup's: lemonfiber's own does not exist yet, and
/// an engine asked about it would answer that nothing is running under it.
#[tokio::test]
async fn adopting_a_setup_that_is_still_running_is_refused_rather_than_captured_live() {
    let env = scratch("running");
    let engine = mounting(somebody_elses(), &["/srv/their-media"]);
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.0",
        400,
        &["media"],
    )]);
    let mut ctx = over(engine, images, Source::External(project()));
    ctx.settings.env_file = Some(env.clone());
    let vault = Arc::new(Captured::default());

    let asked = Command::Migrate(MigrateAction::Act {
        mode: Mode::Adopt,
        confirmed: true,
    });
    let refused = dispatch(asked, &keeping(ctx, &vault)).await;
    assert!(refused.is_err(), "{refused:?}");
    assert!(vault.sources().is_empty(), "it captured a live database");
    assert!(!env.exists(), "a refusal wrote {}", env.display());
}

/// A setup that mounts nothing has nothing to capture, and is adopted without one.
///
/// The alternative would be filing an empty archive as a backup, which reads as
/// protection that was never there.
#[tokio::test]
async fn adopting_a_setup_that_mounts_nothing_takes_no_archive_and_still_goes_through() {
    let env = scratch("unmounted");
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.0",
        400,
        &["media"],
    )]);
    let mut ctx = over(
        somebody_elses_stopped(),
        images,
        Source::External(project()),
    );
    ctx.settings.env_file = Some(env);
    let vault = Arc::new(Captured::default());

    let found = adopting(&keeping(ctx, &vault), true).await;
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Applied),
        "{found:?}"
    );
    assert_eq!(found.and_then(|read| read.backed_up), None);
    assert!(vault.sources().is_empty(), "it captured something");
}
