//! One answer to "which machine is this", for the reads and for the writes.
//!
//! This is the defect the remote work exists to close, and it is worth stating
//! plainly because it shipped. Compose is a subprocess with an inherited
//! environment, so `docker compose` obeyed `DOCKER_HOST` and a named context. The
//! Engine API client resolved its own endpoint and honoured `DOCKER_HOST` only when
//! it named a unix socket, falling back to this machine's daemon otherwise. With a
//! remote context in force, then, the writes went to the server and the reads came
//! from the laptop — a report about one machine and a change to another, with
//! nothing in the output to say so.
//!
//! What closes it is not a check but a shape: one resolved endpoint on the
//! settings, which the client is built from and which the invocation names as
//! `--host`. These hold that shape from both ends — the invocation, and the engine
//! the same value builds — and hold the two refusals that keep it honest when the
//! endpoint cannot be driven at all.

mod common;

use std::path::Path;
use std::sync::Arc;

use common::stack::project;

use lemonfiber_adapters::Daemon;
use lemonfiber_core::app::{dispatch, Command, Ctx};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::error::Diagnose as _;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Engine as _, Health, Lifecycle, Origin, Target};
use lemonfiber_core::ports::process::Output;
use lemonfiber_core::stack::closure::resolve;
use lemonfiber_core::stack::compose::{build, Action};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::support::{Recording, Reporting};
use lemonfiber_manifest::Manifest;

/// The stack this repository carries, so a plan can be resolved without a machine.
const STACK: &str = include_str!("../../../assets/media-stack/stack.toml");

/// A fixed directory, so an invocation does not depend on where it was built.
const STACK_DIR: &str = "/opt/lemonfiber/stack";

/// The invocation these settings produce for the whole stack.
fn invocation(settings: &Settings) -> Vec<String> {
    let Ok(manifest) = Manifest::from_toml(STACK) else {
        unreachable!("the embedded stack parses; the manifest crate asserts it separately");
    };
    let Ok(plan) = resolve(&manifest, &["library".to_owned()], Protocols::both()) else {
        unreachable!("the shipped stack declares that form");
    };
    build(
        &plan,
        settings,
        Path::new(STACK_DIR),
        &Action::Up,
        Environment::MacOs,
    )
}

/// Settings aimed at this engine, with everything else left alone.
fn aimed_at(target: Target) -> Settings {
    Settings {
        protocols: Protocols::both(),
        docker: target,
        ..Settings::default()
    }
}

/// A run over a scripted world, whose engine holds nothing and whose programs all
/// answer the same way.
fn ctx(settings: Settings, runner: &Arc<Recording>) -> Ctx {
    Ctx::new(
        Arc::clone(runner) as Arc<dyn lemonfiber_core::ports::Runner>,
        Arc::new(Reporting::holding(&[], Lifecycle::Running, Health::Healthy)),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: lemonfiber_fixtures::files::Files::empty(),
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        settings,
        Environment::MacOs,
    )
    .rehearsing()
}

/// A run against this machine names nothing, which is what keeps every existing
/// invocation the shape it already had.
#[test]
fn a_run_against_this_machine_names_no_host_at_all() {
    let argv = invocation(&aimed_at(Target::local()));

    assert!(
        !argv.iter().any(|word| word == "--host"),
        "a machine with no context set must be left to its own conventions: {argv:?}"
    );
    assert_eq!(argv.first().map(String::as_str), Some("docker"), "{argv:?}");
    assert_eq!(argv.get(1).map(String::as_str), Some("compose"), "{argv:?}");
}

/// The one property this whole seam exists for: the endpoint Compose is given and
/// the endpoint the engine client is built from are the same value.
///
/// Asserted from both ends of one settings field rather than by comparing two
/// resolutions, because two resolutions agreeing today is what the shipped defect
/// looked like right up until a context was set.
#[test]
fn the_engine_and_the_invocation_are_aimed_at_the_one_endpoint() {
    const ENDPOINT: &str = "ssh://media@nas.local";
    let settings = aimed_at(Target::at(ENDPOINT, Origin::Context("nas".to_owned())));

    let argv = invocation(&settings);
    assert_eq!(argv.first().map(String::as_str), Some("docker"), "{argv:?}");
    assert_eq!(
        argv.get(1).map(String::as_str),
        Some("--host"),
        "the endpoint is a global flag and has to come before the subcommand: {argv:?}"
    );
    assert_eq!(argv.get(2).map(String::as_str), Some(ENDPOINT), "{argv:?}");
    assert_eq!(argv.get(3).map(String::as_str), Some("compose"), "{argv:?}");

    // And the engine seam, built from the same field of the same settings. It says
    // which endpoint it holds, because a bundle that could not would be a bundle
    // that cannot answer this question either.
    let daemon = format!("{:?}", Daemon::reaching(settings.docker.clone()));
    assert!(daemon.contains(ENDPOINT), "{daemon}");
}

/// An endpoint the reads cannot use never becomes one the writes do.
///
/// A mistyped context is the ordinary way in. Falling back to this machine's daemon
/// — which is what the client library does on its own — would answer about the
/// laptop while the operator was asking about the server, so it is refused instead,
/// on both halves and before anything is spawned.
#[tokio::test]
async fn a_context_this_machine_does_not_have_stops_the_reads_and_the_writes() {
    let target = Target::missing("nas");

    let read = Daemon::reaching(target.clone()).list("lemonfiber").await;
    assert_eq!(
        read.err().map(|failure| failure.problem().code.to_string()),
        Some("DOCKER-7".to_owned()),
        "the engine refuses rather than answering about this machine"
    );

    let runner = Arc::new(Recording::answering(Ok(Output {
        status: Some(0),
        stdout: String::new(),
        stderr: String::new(),
    })));
    let refused = dispatch(
        Command::Up { forms: Vec::new() },
        &ctx(aimed_at(target), &runner),
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code.to_string()),
        Some("DOCKER-7".to_owned()),
        "and so does everything that would build an invocation"
    );
    assert!(
        runner.seen().is_empty(),
        "nothing was spawned on the way to refusing: {:?}",
        runner.seen()
    );
}

/// The location the stack mounts is looked for on the machine that will mount it.
///
/// The path is on the laptop, which is exactly why the error an operator meets
/// without this names a directory that is plainly there. The runner answers every
/// command with the exit status a missing directory produces, so what is asserted is
/// that the question was asked at all and that its answer stopped the command.
#[tokio::test]
async fn a_location_that_is_not_on_the_other_machine_stops_the_command() {
    let runner = Arc::new(Recording::answering(Ok(Output {
        status: Some(1),
        stdout: String::new(),
        stderr: String::new(),
    })));
    let settings = Settings {
        data_root: Some(Path::new("/Volumes/media").to_path_buf()),
        ..aimed_at(Target::at("ssh://media@nas.local", Origin::Variable))
    };

    let refused = dispatch(Command::Up { forms: Vec::new() }, &ctx(settings, &runner)).await;

    assert_eq!(
        refused.err().map(|problem| problem.code.to_string()),
        Some("LIFE-5".to_owned())
    );
    let asked = runner.seen();
    let over_ssh = asked.iter().any(|argv| {
        argv.first().is_some_and(|program| program == "ssh")
            && argv.iter().any(|word| word == "media@nas.local")
    });
    assert!(over_ssh, "the other machine was the one asked: {asked:?}");
    assert!(
        !runner.ran("compose"),
        "and nothing was composed against it: {asked:?}"
    );
}
