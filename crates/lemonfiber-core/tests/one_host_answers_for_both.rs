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

use async_trait::async_trait;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::stack::project;

use lemonfiber_adapters::Daemon;
use lemonfiber_core::app::{dispatch, Command, Ctx, ABSENT_THERE};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::error::Diagnose as _;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{
    Engine as _, Health, Lifecycle, Locations, Origin, Presence, Target,
};
use lemonfiber_core::ports::process::Output;
use lemonfiber_core::ports::Narrator;
use lemonfiber_core::stack::closure::resolve;
use lemonfiber_core::stack::compose::{build, Action};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::located::Located;
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

/// Everything the pre-flight said out loud, in the order it said it.
#[derive(Default)]
struct Heard(tokio::sync::Mutex<Vec<String>>);

#[async_trait]
impl Narrator for Heard {
    async fn say(&self, said: &str) {
        self.0.lock().await.push(said.to_owned());
    }
}

impl Heard {
    /// Everything it has heard so far, joined so a case can ask one question of it.
    async fn said(&self) -> String {
        self.0.lock().await.join(" / ")
    }
}

/// A run over a scripted world, whose engine holds nothing and whose programs all
/// answer the same way.
///
/// The machine it would ask about a path is scripted too, and deliberately not the
/// engine fake: what these vary is what is on the machine under the daemon, which is
/// the one question the engine itself is never asked.
fn ctx(settings: Settings, runner: &Arc<Recording>, machine: Arc<dyn Locations>) -> Ctx {
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
    .locating_with(machine)
    .rehearsing()
}

/// A runner that answers everything the same way, since none of these are about it.
fn runner() -> Arc<Recording> {
    Arc::new(Recording::answering(Ok(Output {
        status: Some(0),
        stdout: String::new(),
        stderr: String::new(),
    })))
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
        &ctx(aimed_at(target), &runner, Located::holding(&[])),
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

/// The location the stack mounts is looked for wherever the daemon is, and over
/// whichever transport reaches it.
///
/// TCP is the case this exists for. A daemon answering over a socket offers no shell
/// to put the question to and no key to put it with, which is why this check used to
/// skip it entirely — and an operator with `DOCKER_HOST=tcp://…` got no pre-flight at
/// all while being told nothing about that. The engine is asked instead of a shell,
/// so the endpoint makes no difference to whether the question can be put.
#[tokio::test]
async fn a_location_that_is_not_on_the_other_machine_stops_the_command_over_any_transport() {
    let endpoints = ["tcp://nas.local:2375", "ssh://media@nas.local"];

    for endpoint in endpoints {
        let runner = runner();
        let machine = Located::holding(&[]);
        let settings = Settings {
            data_root: Some(Path::new("/Volumes/media").to_path_buf()),
            ..aimed_at(Target::at(endpoint, Origin::Variable))
        };

        let refused = dispatch(
            Command::Up { forms: Vec::new() },
            &ctx(
                settings,
                &runner,
                Arc::clone(&machine) as Arc<dyn Locations>,
            ),
        )
        .await;

        // Compared against the constant rather than against the number it currently
        // carries: a code moves when two declarations collide, and a test spelling the
        // number is a second place that has to be found when one does.
        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(ABSENT_THERE),
            "{endpoint} must be refused for the location it has not got"
        );
        assert_eq!(
            machine.asked(),
            vec![Path::new("/Volumes/media").to_path_buf()],
            "{endpoint}: the machine was asked about the location, and about nothing else"
        );
        assert!(
            !runner.ran("compose"),
            "{endpoint}: nothing was composed against it: {:?}",
            runner.seen()
        );
    }
}

/// A machine that has the location is let through, and is asked twice before it is.
///
/// The second question is the one worth keeping: it is put about somewhere that
/// cannot be there, and only an answer of "not there" makes the first answer mean
/// anything. See the guard below for what happens when it does not come back.
#[tokio::test]
async fn a_machine_that_has_the_location_is_let_through_on_a_check_that_proved_itself() {
    let runner = runner();
    let machine = Located::holding(&["/srv/media"]);
    let heard = Arc::new(Heard::default());
    let settings = Settings {
        data_root: Some(Path::new("/srv/media").to_path_buf()),
        ..aimed_at(Target::at("tcp://nas.local:2375", Origin::Variable))
    };

    let outcome = dispatch(
        Command::Up { forms: Vec::new() },
        &ctx(
            settings,
            &runner,
            Arc::clone(&machine) as Arc<dyn Locations>,
        )
        .narrating(Arc::clone(&heard) as Arc<dyn Narrator>),
    )
    .await;

    assert_eq!(outcome.err().map(|problem| problem.code), None);
    let asked = machine.asked();
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert_eq!(
        asked.first().map(PathBuf::as_path),
        Some(Path::new("/srv/media"))
    );
    assert_eq!(
        asked.get(1).and_then(|path| path.parent()),
        Some(Path::new("/srv/media")),
        "the control question goes to the same machine under the same location: {asked:?}"
    );
    let said = heard.said().await;
    assert!(
        !said.contains("check") && !said.contains("unverified"),
        "a check that worked has nothing to report about itself: {said}"
    );
}

/// A check that can no longer say no does not get to say yes.
///
/// This is the failure the whole shape guards against, and it cannot be produced by
/// asking a real daemon nicely: the reading rests on the order an engine validates a
/// request in, which nothing promises and a release could change. If it changed,
/// every path would start reading as present and this guard would report green for
/// the rest of its life — so a machine that says yes to somewhere that cannot be
/// there is treated as an instrument that has stopped measuring.
///
/// It lets the command through, because there is nothing wrong with the operator's
/// setup and refusing would be a product that stopped working on a Docker release.
/// It says so instead, and says whose fault it is, because that reader is not the
/// operator.
#[tokio::test]
async fn a_machine_that_says_yes_to_everywhere_is_not_believed_about_anywhere() {
    let runner = runner();
    let heard = Arc::new(Heard::default());
    let settings = Settings {
        data_root: Some(Path::new("/srv/media").to_path_buf()),
        ..aimed_at(Target::at("ssh://media@nas.local", Origin::Variable))
    };

    let outcome = dispatch(
        Command::Up { forms: Vec::new() },
        &ctx(settings, &runner, Located::saying(Presence::There))
            .narrating(Arc::clone(&heard) as Arc<dyn Narrator>),
    )
    .await;

    assert_eq!(
        outcome.err().map(|problem| problem.code),
        None,
        "a check that has stopped working is not a reason to refuse a working machine"
    );
    let said = heard.said().await;
    assert!(
        said.contains("stopped working") && said.contains("fault in lemonfiber"),
        "the run is told the check failed rather than the location: {said}"
    );
    assert!(
        said.contains("/srv/media") && said.contains("nas.local"),
        "{said}"
    );
}

/// A machine that answers something neither yes nor no is let through, and said so.
///
/// The specification has a state for a remote context whose files are not confirmed,
/// and this is it. What it must not be is silent: an operator whose pre-flight did
/// not happen and who was never told is in exactly the position the pre-flight was
/// written to keep them out of.
#[tokio::test]
async fn a_run_that_could_not_be_checked_is_let_through_and_told_so() {
    let runner = runner();
    let heard = Arc::new(Heard::default());
    let settings = Settings {
        data_root: Some(Path::new("/srv/media").to_path_buf()),
        ..aimed_at(Target::at("tcp://nas.local:2375", Origin::Variable))
    };

    let outcome = dispatch(
        Command::Up { forms: Vec::new() },
        &ctx(settings, &runner, Located::saying(Presence::Unknown))
            .narrating(Arc::clone(&heard) as Arc<dyn Narrator>),
    )
    .await;

    assert_eq!(outcome.err().map(|problem| problem.code), None);
    let said = heard.said().await;
    assert!(
        said.contains("could not check") && said.contains("empty directory"),
        "an unverified run is told what it is risking: {said}"
    );
    assert!(
        !said.contains("fault in lemonfiber"),
        "and is not sent to report a fault that may well be its own daemon: {said}"
    );
}

/// A machine that cannot be reached is refused here rather than three commands later.
///
/// The distinctions the connection draws — a name that went nowhere, a port that
/// declined, a key that was refused — are worth more in front of the command than
/// inside whatever Compose eventually prints.
#[tokio::test]
async fn a_machine_that_cannot_be_reached_is_refused_in_the_engines_own_words() {
    let runner = runner();
    let settings = Settings {
        data_root: Some(Path::new("/srv/media").to_path_buf()),
        ..aimed_at(Target::at("ssh://media@nas.local", Origin::Variable))
    };

    let refused = dispatch(
        Command::Up { forms: Vec::new() },
        &ctx(settings, &runner, Located::unreachable("no route to host")),
    )
    .await;

    assert!(
        refused.is_err(),
        "a machine that will not answer is not a machine to start a stack on"
    );
    assert!(
        !runner.ran("compose"),
        "and nothing was composed against it: {:?}",
        runner.seen()
    );
}

/// An operator who has not chosen a location yet has nothing to be checked.
///
/// The pre-flight asks about the location the stack mounts, so with no location
/// there is no question — and inventing one would refuse a machine over a setting
/// the operator has not made.
#[tokio::test]
async fn a_run_with_no_location_chosen_asks_the_machine_nothing() {
    let runner = runner();
    let machine = Located::holding(&[]);

    let outcome = dispatch(
        Command::Up { forms: Vec::new() },
        &ctx(
            aimed_at(Target::at("tcp://nas.local:2375", Origin::Variable)),
            &runner,
            Arc::clone(&machine) as Arc<dyn Locations>,
        ),
    )
    .await;

    assert_eq!(outcome.err().map(|problem| problem.code), None);
    assert!(machine.asked().is_empty(), "{:?}", machine.asked());
}
