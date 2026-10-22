//! The project lemonfiber leaves in the project directory runs under plain Compose as
//! lemonfiber runs it.
//!
//! `docker compose config` resolves a project the way `up` would start it, so the two
//! are compared through it: lemonfiber's own invocation, and plain Compose in the project
//! directory with nothing but what lemonfiber wrote there. Asked of the `docker` on the
//! machine running the tests, because what is claimed is what that program reads.

use std::path::{Path, PathBuf};
use std::process::Command as Program;
use std::sync::Arc;
use std::time::Duration;

use include_dir::{Dir, DirEntry, File};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::process::Output;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::networked::Networked;
use lemonfiber_fixtures::ports::Following;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_fixtures::support::{Reporting, Scripted};
use lemonfiber_fixtures::volumed::Volumed;

/// What the operator's environment file holds beyond lemonfiber's own: the values the
/// project is written with, and a secret it is not.
const ENVIRONMENT: &str = "PUID=1001\nPGID=1002\nTZ=Pacific/Auckland\nLAN_BIND=192.168.1.5\nWIREGUARD_PRIVATE_KEY=never-in-the-project\n";

/// A machine running the stack this build carries, its settings in a directory of their
/// own.
fn machine(name: &str, overlay: Option<&str>) -> (Ctx, PathBuf) {
    let config = Scratch::new(name).kept().canonicalize().unwrap_or_default();
    let env = config.join(".env");
    let data = config.join("data");
    let _ = std::fs::create_dir_all(&data);
    let _ = std::fs::write(&env, format!("{ENVIRONMENT}DATA_ROOT={}\n", data.display()));
    let overlays: Vec<PathBuf> = overlay
        .map(|text| {
            let at = lemonfiber_core::overlay::at(&env);
            let _ = std::fs::write(&at, text);
            at
        })
        .into_iter()
        .collect();
    let directory = config.join("stack");
    let stack = carried();
    let ids: Vec<String> = stack
        .manifest()
        .map(|manifest| {
            manifest
                .services
                .into_iter()
                .map(|service| service.id)
                .collect()
        })
        .unwrap_or_default();
    let running: Vec<&str> = ids.iter().map(String::as_str).collect();
    let ctx = lemonfiber_testing::a_context()
        .over(stack)
        .runner(Arc::new(Scripted(Ok(Output {
            status: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        }))))
        .engine(Arc::new(Reporting::holding(
            &running,
            Lifecycle::Running,
            Health::Healthy,
        )))
        .clock(Following::started())
        .networks(Networked::none())
        .engine_volumes(Volumed::none())
        .settings(Settings {
            protocols: Protocols::both(),
            env_file: Some(env),
            overlays,
            stack_dir: Some(directory.clone()),
            ..Settings::default()
        })
        .build()
        .with_patience(Duration::from_secs(180));
    (ctx, directory)
}

/// The stack the bundle declares, carried the way this binary carries it: its manifest
/// written apart as a root and a file per service.
fn carried() -> Source {
    let Ok((root, services)) = lemonfiber_manifest::assembly::split(lemonfiber_bundled::STACK)
    else {
        unreachable!("the stack the bundle declares is written apart")
    };
    let held: Vec<DirEntry<'static>> = services
        .into_iter()
        .map(|(file, text)| {
            DirEntry::File(File::new(
                leaked(format!(
                    "{}/{file}",
                    lemonfiber_manifest::assembly::SERVICES
                )),
                leaked(text).as_bytes(),
            ))
        })
        .collect();
    let entries = vec![
        DirEntry::File(File::new(
            lemonfiber_manifest::assembly::ROOT,
            leaked(root).as_bytes(),
        )),
        DirEntry::Dir(Dir::new(
            lemonfiber_manifest::assembly::SERVICES,
            Box::leak(held.into_boxed_slice()),
        )),
    ];
    Source::Embedded(Box::leak(Box::new(Dir::new(
        "",
        Box::leak(entries.into_boxed_slice()),
    ))))
}

/// `text`, held for as long as the tests run.
fn leaked(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// What `docker compose` resolves `arguments` to, run in `at`.
fn resolved(arguments: &[String], at: &Path) -> String {
    let ran = Program::new("docker")
        .args(arguments)
        .current_dir(at)
        .output();
    let Ok(ran) = ran else {
        unreachable!("docker is installed where these tests run: {ran:?}")
    };
    assert!(
        ran.status.success(),
        "docker {} failed: {}",
        arguments.join(" "),
        String::from_utf8_lossy(&ran.stderr)
    );
    String::from_utf8_lossy(&ran.stdout).into_owned()
}

/// lemonfiber's invocation of `action`, asking Compose for the project it resolves to
/// instead.
fn configured(command: &[String], action: &str) -> Vec<String> {
    let at = command
        .iter()
        .position(|word| word == action)
        .unwrap_or(command.len());
    command
        .iter()
        .skip(1)
        .take(at - 1)
        .cloned()
        .chain(std::iter::once("config".to_owned()))
        .collect()
}

/// What `docker compose config` resolves lemonfiber's own start to, and what plain
/// `docker compose config` in the project directory resolves.
async fn both(ctx: &Ctx, project: &Path) -> (String, String) {
    let outcome = dispatch(Command::Up { forms: Vec::new() }, ctx).await;
    let Ok(Outcome::Lifecycle(report)) = outcome else {
        unreachable!("the start ran: {outcome:?}")
    };
    let ours = resolved(&configured(&report.command, "up"), Path::new("/"));
    let plain = resolved(&["compose".to_owned(), "config".to_owned()], project);
    (ours, plain)
}

#[tokio::test(start_paused = true)]
async fn plain_compose_in_the_project_directory_resolves_what_lemonfiber_started() {
    let (ctx, project) = machine("plain-compose", None);

    let (ours, plain) = both(&ctx, &project).await;

    assert_eq!(plain, ours);
    assert!(ours.contains("name: lemonfiber"), "{ours}");
    assert!(
        ours.contains("jellyfin:") && ours.contains("gluetun:"),
        "{ours}"
    );
    let written =
        std::fs::read_to_string(project.join(lemonfiber_core::project::FILE)).unwrap_or_default();
    let beside = std::fs::read_to_string(project.join(".env")).unwrap_or_default();
    assert!(
        written.contains("env_file:"),
        "a secret is read from its own file: {written}"
    );
    assert!(
        !written.contains("${"),
        "nothing is left for lemonfiber to fill: {written}"
    );
    assert_eq!(
        beside
            .lines()
            .map(|line| line.split('=').next().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["COMPOSE_PROFILES", "COMPOSE_FILE"],
        "{beside}"
    );
    assert!(
        !written.contains("never-in-the-project") && !beside.contains("never-in-the-project"),
        "no secret is written in"
    );
}

#[tokio::test(start_paused = true)]
async fn an_overlay_overrides_the_project_under_plain_compose_as_it_does_under_lemonfiber() {
    let (ctx, project) = machine(
        "plain-compose-overlay",
        Some("services:\n  jellyfin:\n    environment:\n      TZ: Europe/Lisbon\n"),
    );

    let (ours, plain) = both(&ctx, &project).await;

    assert_eq!(plain, ours);
    assert!(
        ours.contains("TZ: Europe/Lisbon"),
        "the overlay wins: {ours}"
    );
    assert!(
        ours.contains("TZ: Pacific/Auckland"),
        "and only where it says: {ours}"
    );
}

/// Every service of every profile, each secrets file it names among them, resolves from
/// what a reset alone leaves.
#[tokio::test(start_paused = true)]
async fn the_project_a_reset_writes_resolves_under_plain_compose() {
    let (ctx, project) = machine("plain-reset", None);

    let outcome = dispatch(Command::Reset { confirm: true }, &ctx).await;

    assert!(outcome.is_ok(), "{outcome:?}");
    let plain = resolved(
        &["compose", "--profile", "*", "config"].map(str::to_owned),
        &project,
    );
    assert!(plain.contains("name: lemonfiber"), "{plain}");
}
