//! A copy of lemonfiber in a container starts a stack only where the machine under it
//! has the stack's paths at the same paths.
//!
//! Compose resolves every bind mount on the machine the engine runs on. A copy in a
//! container hands it the paths the container sees, so a stack directory mounted from
//! somewhere else is a stack the engine mounts from the wrong place. These drive a
//! start through the dispatcher with the engine scripted to describe the container,
//! and hold that a start is refused, naming both paths, before anything is composed.

use std::path::Path;
use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Ctx};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::error::codes::life::{
    ELSEWHERE_UNDERNEATH, NOT_ON_THIS_ENGINE, NO_ENGINE_IN_HERE,
};
use lemonfiber_core::ports::docker::{Health, Lifecycle, Locations, Origin, Target};
use lemonfiber_core::ports::process::Output;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::located::Located;
use lemonfiber_fixtures::support::{Recording, Reporting};

/// The container the copy runs in, as the engine names it.
const ID: &str = "4f1c0d7e9a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5";

/// Where the stack lives, inside the container and on a NAS that mounts it there.
const STACK: &str = "/mnt/user/appdata/lemonfiber/stack";

/// Where the media lives, inside and out.
const MEDIA: &str = "/mnt/user/data";

/// Settings for a copy running in the container, over a chosen data root.
fn inside() -> Settings {
    Settings {
        protocols: Protocols::both(),
        container: Some(ID.to_owned()),
        data_root: Some(Path::new(MEDIA).to_path_buf()),
        ..Settings::default()
    }
}

/// A run over a scripted world whose machine describes the container this way.
fn ctx(
    settings: Settings,
    runner: &Arc<Recording>,
    machine: Arc<dyn Locations>,
    stack: Source,
) -> Ctx {
    lemonfiber_testing::a_context()
        .runner(Arc::clone(runner) as Arc<dyn lemonfiber_core::ports::Runner>)
        .engine(Arc::new(Reporting::holding(
            &[],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .filesystem(lemonfiber_fixtures::files::Files::empty())
        .over(stack)
        .settings(settings)
        .build()
        .with_locations(machine)
}

/// A runner that answers everything the same way, since none of these are about it.
fn runner() -> Arc<Recording> {
    Arc::new(Recording::answering(Ok(Output {
        status: Some(0),
        stdout: String::new(),
        stderr: String::new(),
    })))
}

/// Start the whole stack under these settings, against this machine.
async fn started(
    settings: Settings,
    machine: Arc<Located>,
) -> (Option<lemonfiber_core::error::Problem>, Arc<Recording>) {
    let runner = runner();
    let outcome = dispatch(
        Command::Up { forms: Vec::new() },
        &ctx(
            settings,
            &runner,
            machine as Arc<dyn Locations>,
            Source::External(Path::new(STACK)),
        ),
    )
    .await;
    (outcome.err().map(|problem| *problem), runner)
}

#[tokio::test]
async fn a_stack_directory_mounted_from_another_path_is_refused_naming_both() {
    let machine = Located::running(
        ID,
        &[
            ("/mnt/cache/appdata/lemonfiber/stack", STACK),
            (MEDIA, MEDIA),
        ],
    );

    let (refused, runner) = started(inside(), machine).await;

    let problem = refused.unwrap_or_else(|| unreachable!("a start over the wrong path went ahead"));
    assert_eq!(problem.code, ELSEWHERE_UNDERNEATH);
    assert!(problem.summary.contains(STACK), "{problem:?}");
    assert!(
        problem
            .summary
            .contains("/mnt/cache/appdata/lemonfiber/stack"),
        "{problem:?}"
    );
    assert!(!runner.ran("compose"), "{:?}", runner.seen());
}

#[tokio::test]
async fn a_stack_directory_that_is_only_inside_the_container_is_refused() {
    let machine = Located::running(ID, &[(MEDIA, MEDIA)]);

    let (refused, runner) = started(inside(), machine).await;

    let problem =
        refused.unwrap_or_else(|| unreachable!("a start over the container's layer went ahead"));
    assert_eq!(problem.code, ELSEWHERE_UNDERNEATH);
    assert!(problem.summary.contains(STACK), "{problem:?}");
    assert!(!runner.ran("compose"), "{:?}", runner.seen());
}

/// The data root is resolved on the machine the same way, so it is held to the same.
#[tokio::test]
async fn a_data_root_mounted_from_another_path_is_refused_naming_both() {
    let machine = Located::running(ID, &[(STACK, STACK), ("/mnt/disk1/data", MEDIA)]);

    let (refused, runner) = started(inside(), machine).await;

    let problem =
        refused.unwrap_or_else(|| unreachable!("a start over the wrong data root went ahead"));
    assert_eq!(problem.code, ELSEWHERE_UNDERNEATH);
    assert!(problem.summary.contains(MEDIA), "{problem:?}");
    assert!(problem.summary.contains("/mnt/disk1/data"), "{problem:?}");
    assert!(!runner.ran("compose"), "{:?}", runner.seen());
}

/// A container whose paths are the machine's paths is let through. Rehearsed, so what
/// is proved is that the guard passed rather than what a real start then did.
#[tokio::test]
async fn a_container_that_shares_its_paths_with_the_machine_is_let_through() {
    let runner = runner();
    let stack = lemonfiber_testing::repository_stack();
    let Source::External(at) = stack else {
        unreachable!("the repository's stack is read from disk");
    };
    let at = at.display().to_string();
    let machine = Located::running(ID, &[(at.as_str(), at.as_str()), (MEDIA, MEDIA)]);

    let outcome = dispatch(
        Command::Up { forms: Vec::new() },
        &ctx(inside(), &runner, machine as Arc<dyn Locations>, stack).rehearsing(),
    )
    .await;

    assert_eq!(outcome.err().map(|problem| problem.code), None);
}

/// No socket in the container is said as such, and nothing starts.
#[tokio::test]
async fn a_container_with_no_way_to_docker_says_so_and_starts_nothing() {
    let machine = Located::unreachable("connect: no such file or directory");

    let (refused, runner) = started(inside(), machine).await;

    assert_eq!(refused.map(|problem| problem.code), Some(NO_ENGINE_IN_HERE));
    assert!(!runner.ran("compose"), "{:?}", runner.seen());
}

/// An engine that does not know the container cannot be asked about its paths, and a
/// check that could not be made refuses rather than passes.
#[tokio::test]
async fn an_engine_that_does_not_know_the_container_refuses_rather_than_passes() {
    let machine = Located::running("someone-else", &[(STACK, STACK), (MEDIA, MEDIA)]);

    let (refused, runner) = started(inside(), machine).await;

    assert_eq!(
        refused.map(|problem| problem.code),
        Some(NOT_ON_THIS_ENGINE)
    );
    assert!(!runner.ran("compose"), "{:?}", runner.seen());
}

/// Outside a container nothing is asked, and an engine that could not have answered
/// stops nothing.
#[tokio::test]
async fn a_copy_that_is_not_in_a_container_asks_nothing_of_the_engine() {
    let runner = runner();
    let settings = Settings {
        container: None,
        ..inside()
    };

    let outcome = dispatch(
        Command::Up { forms: Vec::new() },
        &ctx(
            settings,
            &runner,
            Located::unreachable("no engine") as Arc<dyn Locations>,
            lemonfiber_testing::repository_stack(),
        )
        .rehearsing(),
    )
    .await;

    assert_eq!(outcome.err().map(|problem| problem.code), None);
}

/// A remote engine's machine is asked about directly by its own guard, so the one
/// under the container does not apply.
#[tokio::test]
async fn a_remote_engine_is_left_to_the_remote_guard() {
    let runner = runner();
    let settings = Settings {
        docker: Target::at("tcp://nas.local:2375", Origin::Variable),
        data_root: None,
        ..inside()
    };

    let outcome = dispatch(
        Command::Up { forms: Vec::new() },
        &ctx(
            settings,
            &runner,
            Located::running("someone-else", &[]) as Arc<dyn Locations>,
            lemonfiber_testing::repository_stack(),
        )
        .rehearsing(),
    )
    .await;

    assert_eq!(outcome.err().map(|problem| problem.code), None);
}
