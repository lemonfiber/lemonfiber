use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use super::{
    docker_version, parse_version, Category, Check, EnvironmentCheck, Verdict, API_MISMATCH,
    COMPOSE_UNUSABLE, DAEMON_DOWN, DOCKER_ABSENT,
};
use crate::ports::docker::{Origin, Target};
use crate::ports::process::{Failure, Output, Runner};

/// A runner that answers each argv with whatever the test scripted for it,
/// keyed by the program-plus-subcommand so `docker version` and `docker
/// compose version` can be answered differently in one run.
#[derive(Default)]
struct Bench {
    answers: HashMap<String, Result<Output, Failure>>,
}

impl Bench {
    /// The key an argv is filed under: enough of it to tell the two commands
    /// this check runs apart.
    fn key(argv: &[String]) -> String {
        argv.iter().take(2).cloned().collect::<Vec<_>>().join(" ")
    }

    /// Script the Docker client's answer.
    fn docker(mut self, answer: Result<Output, Failure>) -> Self {
        self.answers.insert("docker version".to_owned(), answer);
        self
    }

    /// Script the Compose plugin's answer.
    fn compose(mut self, answer: Result<Output, Failure>) -> Self {
        self.answers.insert("docker compose".to_owned(), answer);
        self
    }
}

#[async_trait]
impl Runner for Bench {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        match self.answers.get(&Self::key(argv)) {
            Some(Ok(output)) => Ok(output.clone()),
            Some(Err(Failure::NotFound { program })) => Err(Failure::NotFound {
                program: program.clone(),
            }),
            Some(Err(Failure::Unusable { program, reason })) => Err(Failure::Unusable {
                program: program.clone(),
                reason: reason.clone(),
            }),
            None => Err(Failure::NotFound {
                program: argv.first().cloned().unwrap_or_default(),
            }),
        }
    }
}

fn spoke(stdout: &str) -> Output {
    Output {
        status: Some(0),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

fn refused(stderr: &str) -> Output {
    Output {
        status: Some(1),
        stdout: String::new(),
        stderr: stderr.to_owned(),
    }
}

async fn run(bench: Bench) -> Vec<super::Finding> {
    EnvironmentCheck::new(Arc::new(bench)).run().await
}

/// The verdict for one of the check's two findings, absent only if the check
/// stopped reporting both — which the assertions treat as a failure.
fn verdict<'a>(findings: &'a [super::Finding], check: &str) -> Option<&'a Verdict> {
    findings
        .iter()
        .find(|finding| finding.check == check)
        .map(|finding| &finding.verdict)
}

#[tokio::test]
async fn a_healthy_machine_passes_both_findings_and_shows_the_versions() {
    let bench = Bench::default()
        .docker(Ok(spoke("27.1.1\n")))
        .compose(Ok(spoke("v2.32.1\n")));
    let findings = run(bench).await;

    assert!(matches!(
        verdict(&findings, "environment.engine"),
        Some(Verdict::Pass { note: Some(note) }) if note == "27.1.1"
    ));
    assert!(matches!(
        verdict(&findings, "environment.compose"),
        Some(Verdict::Pass { note: Some(note) }) if note == "v2.32.1"
    ));
    assert!(findings
        .iter()
        .all(|finding| finding.category == Category::Environment));
}

#[tokio::test]
async fn an_absent_client_fails_distinctly_and_skips_compose() {
    let bench = Bench::default().docker(Err(Failure::NotFound {
        program: "docker".to_owned(),
    }));
    let findings = run(bench).await;

    assert!(matches!(
        verdict(&findings, "environment.engine"),
        Some(Verdict::Fail(problem)) if problem.code == DOCKER_ABSENT
    ));
    // Compose is not a failure when there is no Docker to run it through.
    assert!(matches!(
        verdict(&findings, "environment.compose"),
        Some(Verdict::Skipped { reason }) if reason.contains("not installed")
    ));
}

#[tokio::test]
async fn a_present_client_with_a_dead_daemon_is_a_different_failure() {
    // Runs, names no server, and says why on stderr — the shape of a client
    // whose daemon is stopped, which must stay distinct from absence.
    let bench = Bench::default()
        .docker(Ok(refused("Cannot connect to the Docker daemon")))
        .compose(Ok(spoke("v2.32.1")));
    let findings = run(bench).await;

    assert!(matches!(
        verdict(&findings, "environment.engine"),
        Some(Verdict::Fail(problem))
            if problem.code == DAEMON_DOWN
                && problem.detail.as_deref() == Some("Cannot connect to the Docker daemon")
    ));
    // The plugin does not need the daemon, so its version is still checked.
    assert!(matches!(
        verdict(&findings, "environment.compose"),
        Some(Verdict::Pass { .. })
    ));
}

#[tokio::test]
async fn a_daemon_that_says_nothing_still_gets_a_reason() {
    let bench = Bench::default()
        .docker(Ok(refused("")))
        .compose(Ok(spoke("2.5.0")));
    let findings = run(bench).await;
    assert!(matches!(
        verdict(&findings, "environment.engine"),
        Some(Verdict::Fail(problem)) if problem.detail.as_deref() == Some("the daemon did not answer")
    ));
}

#[tokio::test]
async fn a_client_that_will_not_start_is_reported_and_skips_compose() {
    let bench = Bench::default().docker(Err(Failure::Unusable {
        program: "docker".to_owned(),
        reason: "permission denied".to_owned(),
    }));
    let findings = run(bench).await;
    assert!(matches!(
        verdict(&findings, "environment.engine"),
        Some(Verdict::Fail(problem))
            if problem.code == DAEMON_DOWN
                && problem.detail.as_deref() == Some("permission denied")
                // Guided, the same as its daemon-down sibling — both point the
                // operator to act elsewhere, so they must agree.
                && problem.state == crate::error::State::Guided
    ));
    assert!(matches!(
        verdict(&findings, "environment.compose"),
        Some(Verdict::Skipped { reason }) if reason.contains("would not start")
    ));
}

/// Both numbers, from the one call, whether they agree or not.
///
/// The pair is the point. An operator told the daemon speaks something older has
/// no idea whether that is one release behind or six, and cannot act on it.
#[tokio::test]
async fn the_two_ends_api_versions_are_both_reported() {
    let agreed = Bench::default()
        .docker(Ok(spoke("27.1.1|1.47|1.47\n")))
        .compose(Ok(spoke("v2.32.1")));
    let findings = run(agreed).await;
    let agreed_on = verdict(&findings, "environment.api");
    assert!(
        matches!(
            agreed_on,
            Some(Verdict::Pass { note: Some(note) }) if note == "1.47"
        ),
        "{agreed_on:?}"
    );
    // The engine finding still shows the daemon's own release, unchanged by the
    // two fields that now ride along with it.
    assert!(matches!(
        verdict(&findings, "environment.engine"),
        Some(Verdict::Pass { note: Some(note) }) if note == "27.1.1"
    ));

    let apart = Bench::default()
        .docker(Ok(spoke("24.0.7|1.51|1.43")))
        .compose(Ok(spoke("v2.32.1")));
    let findings = run(apart).await;
    let said = verdict(&findings, "environment.api");
    assert!(
        matches!(
            said,
            Some(Verdict::Warn(problem))
                if problem.code == API_MISMATCH
                    && problem.summary.contains("1.51")
                    && problem.summary.contains("1.43")
        ),
        "both versions are named: {said:?}"
    );
}

/// A mismatch is a difference in what is available rather than a fault, so the
/// run is not failed by it.
#[tokio::test]
async fn a_mismatch_is_reported_without_failing_the_run() {
    let findings = run(Bench::default()
        .docker(Ok(spoke("24.0.7|1.51|1.43")))
        .compose(Ok(spoke("v2.32.1"))))
    .await;
    assert!(!matches!(
        verdict(&findings, "environment.api"),
        Some(Verdict::Fail(_))
    ));
}

/// A client that names no API version at all leaves the question open rather
/// than answering it with an empty string.
#[tokio::test]
async fn a_client_that_states_no_api_version_settles_nothing() {
    let findings = run(Bench::default()
        .docker(Ok(spoke("27.1.1")))
        .compose(Ok(spoke("v2.32.1"))))
    .await;
    assert!(matches!(
        verdict(&findings, "environment.api"),
        Some(Verdict::Unverified { .. })
    ));

    let down = run(Bench::default()
        .docker(Ok(refused("Cannot connect to the Docker daemon")))
        .compose(Ok(spoke("v2.32.1"))))
    .await;
    assert!(matches!(
        verdict(&down, "environment.api"),
        Some(Verdict::Skipped { .. })
    ));
}

#[tokio::test]
async fn a_missing_compose_plugin_fails_where_docker_is_present() {
    let bench = Bench::default()
        .docker(Ok(spoke("27.1.1")))
        .compose(Ok(refused("docker: 'compose' is not a docker command")));
    let findings = run(bench).await;
    assert!(matches!(
        verdict(&findings, "environment.compose"),
        Some(Verdict::Fail(problem)) if problem.code == COMPOSE_UNUSABLE
    ));
}

#[tokio::test]
async fn a_compose_below_the_floor_fails_and_names_what_it_found() {
    let bench = Bench::default()
        .docker(Ok(spoke("27.1.1")))
        .compose(Ok(spoke("1.29.2")));
    let findings = run(bench).await;
    assert!(matches!(
        verdict(&findings, "environment.compose"),
        Some(Verdict::Fail(problem))
            if problem.code == COMPOSE_UNUSABLE && problem.summary.contains("1.29.2")
    ));
}

#[tokio::test]
async fn a_compose_that_will_not_run_is_unverified_not_failed() {
    let bench = Bench::default()
        .docker(Ok(spoke("27.1.1")))
        .compose(Err(Failure::Unusable {
            program: "docker".to_owned(),
            reason: "boom".to_owned(),
        }));
    let findings = run(bench).await;
    assert!(matches!(
        verdict(&findings, "environment.compose"),
        Some(Verdict::Unverified { .. })
    ));
}

#[tokio::test]
async fn a_compose_version_that_cannot_be_read_is_unverified() {
    let bench = Bench::default()
        .docker(Ok(spoke("27.1.1")))
        .compose(Ok(spoke("not-a-version")));
    let findings = run(bench).await;
    assert!(matches!(
        verdict(&findings, "environment.compose"),
        Some(Verdict::Unverified { reason, .. }) if reason.contains("could not be read")
    ));
}

/// The versions have to describe the daemon the stack is actually run against.
///
/// A client left to work out which daemon it means is a second opinion about
/// which machine this is, and it is wrong in exactly the case the numbers are
/// being asked for: two machines, updated at different times. So the endpoint is
/// named on the invocation, ahead of the subcommand, where it is a global flag.
#[test]
fn the_versions_are_asked_of_the_daemon_the_stack_is_run_against() {
    let here = docker_version(&Target::local());
    assert_eq!(here.first().map(String::as_str), Some("docker"));
    assert_eq!(here.get(1).map(String::as_str), Some("version"));
    assert!(
        !here.iter().any(|word| word == "--host"),
        "a local run is left to this machine's own conventions: {here:?}"
    );

    let there = docker_version(&Target::at("ssh://media@nas.local", Origin::Variable));
    assert_eq!(
        there.iter().take(4).cloned().collect::<Vec<_>>(),
        vec![
            "docker".to_owned(),
            "--host".to_owned(),
            "ssh://media@nas.local".to_owned(),
            "version".to_owned(),
        ],
        "{there:?}"
    );
    assert!(
        there
            .last()
            .is_some_and(|format| format.contains("Client.APIVersion")
                && format.contains("Server.APIVersion")),
        "both ends are asked for in the one call: {there:?}"
    );
}

#[test]
fn a_version_is_read_leniently_and_compared_on_what_matters() {
    assert_eq!(parse_version("v2.32.1\n"), Some((2, 32, 1)));
    assert_eq!(parse_version("2.5"), Some((2, 5, 0)));
    assert_eq!(parse_version("27"), Some((27, 0, 0)));
    assert_eq!(parse_version("2.6.1-desktop.1"), Some((2, 6, 1)));
    assert_eq!(parse_version(""), None);
    assert_eq!(parse_version("nonsense"), None);
}
