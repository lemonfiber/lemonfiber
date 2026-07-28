//! Proving the machine can run the stack before anything is started.
//!
//! Three things have to be true before a form can come up: the Docker client is
//! installed, its daemon is answering, and the Compose plugin is new enough to
//! drive. The wizard asks the same questions setup does, so the two share this
//! check rather than each testing the ground in its own way.
//!
//! "Docker is missing" and "Docker is here but its daemon is down" are different
//! problems with different remedies, and the honest way to tell them apart is to
//! run the client and watch which way it fails: not found at all, or found and
//! unable to reach the daemon it fronts.
//!
//! See `.docs/architecture/module-layout.md`.

use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::ports::process::Failure;
use crate::ports::Runner;

/// Raised when the Docker client is not installed.
pub const DOCKER_ABSENT: Code = Code::new("ENV-1");

/// Raised when the Docker client is present but its daemon is not answering.
pub const DAEMON_DOWN: Code = Code::new("ENV-2");

/// Raised when the Compose plugin is missing or too old to drive.
pub const COMPOSE_UNUSABLE: Code = Code::new("ENV-3");

/// The oldest Compose the driver is willing to build against.
///
/// The driver speaks to the v2 plugin — `docker compose`, one word apart from
/// the end-of-life `docker-compose` v1, which is a different program with
/// different profile semantics. So the floor is the first v2: below it the
/// commands this builds would not be understood at all.
const MINIMUM_COMPOSE: (u32, u32, u32) = (2, 0, 0);

/// Whether the machine has an engine to run the stack, and a Compose new enough
/// to drive it.
pub struct EnvironmentCheck {
    runner: Arc<dyn Runner>,
}

/// What running the Docker client revealed about the engine.
enum Engine {
    /// The client answered and named its daemon's version.
    Up(String),
    /// The client is not installed.
    Absent,
    /// The client is present but its daemon did not answer.
    DaemonDown(String),
    /// The client is installed but the operating system would not start it.
    Unusable(String),
}

impl EnvironmentCheck {
    /// A check over the given way of running programs.
    #[must_use]
    pub fn new(runner: Arc<dyn Runner>) -> Self {
        Self { runner }
    }

    /// Ask the Docker client for its daemon's version, and read the answer for
    /// which of the four things it means.
    async fn engine(&self) -> Engine {
        match self.runner.run(&docker_version()).await {
            Err(Failure::NotFound { .. }) => Engine::Absent,
            Err(Failure::Unusable { reason, .. }) => Engine::Unusable(reason),
            Ok(output) => {
                let version = output.stdout.trim();
                if output.succeeded() && !version.is_empty() {
                    Engine::Up(version.to_owned())
                } else {
                    // A client that ran and yet named no server is a client whose
                    // daemon did not answer; its own words on stderr are the
                    // nearest thing to a reason.
                    Engine::DaemonDown(reason(&output.stderr))
                }
            }
        }
    }

    /// Ask the Compose plugin for its version, and judge it against the floor.
    ///
    /// Only meaningful once the client is present, so the caller runs it only
    /// then — a Compose that cannot be reached because Docker is absent is a
    /// skipped check, not a failed one.
    async fn compose(&self) -> Verdict {
        match self.runner.run(&compose_version()).await {
            Err(_) => unverified("the Compose plugin could not be run"),
            Ok(output) if !output.succeeded() => Verdict::Fail(compose_absent()),
            Ok(output) => match parse_version(&output.stdout) {
                None => unverified("the Compose plugin reported a version that could not be read"),
                Some(found) if found >= MINIMUM_COMPOSE => Verdict::Pass {
                    note: Some(output.stdout.trim().to_owned()),
                },
                Some(found) => Verdict::Fail(compose_outdated(&describe(found))),
            },
        }
    }
}

#[async_trait]
impl Check for EnvironmentCheck {
    fn category(&self) -> Category {
        Category::Environment
    }

    async fn run(&self) -> Vec<Finding> {
        let engine = self.engine().await;
        let compose = match &engine {
            // With no client there is nothing to run Compose through, so it is
            // skipped with the same reason the operator will act on first.
            Engine::Absent => Verdict::Skipped {
                reason: "Docker is not installed".to_owned(),
            },
            Engine::Unusable(_) => Verdict::Skipped {
                reason: "the Docker client would not start".to_owned(),
            },
            Engine::Up(_) | Engine::DaemonDown(_) => self.compose().await,
        };

        vec![
            finding(
                "environment.engine",
                "Docker engine",
                engine_verdict(engine),
            ),
            finding("environment.compose", "Docker Compose", compose),
        ]
    }
}

/// The command that asks the client for the version of the daemon it fronts.
///
/// The server field is the point: the client prints its own version without a
/// daemon, and only names the server once one has answered.
fn docker_version() -> Vec<String> {
    ["docker", "version", "--format", "{{.Server.Version}}"]
        .map(str::to_owned)
        .to_vec()
}

/// The command that asks the Compose plugin for its bare version number.
fn compose_version() -> Vec<String> {
    ["docker", "compose", "version", "--short"]
        .map(str::to_owned)
        .to_vec()
}

/// The engine finding, from what running the client revealed.
fn engine_verdict(engine: Engine) -> Verdict {
    match engine {
        Engine::Up(version) => Verdict::Pass {
            note: Some(version),
        },
        Engine::Absent => Verdict::Fail(
            Problem::new(
                DOCKER_ABSENT,
                Severity::Error,
                "Docker is not installed",
                "lemonfiber runs your stack in containers, so a container engine has to be installed before any of it can start.",
                Remedy::new("Install Docker Desktop, or Docker Engine on Linux")
                    .with_detail("https://docs.docker.com/get-started/get-docker/"),
            )
            .in_state(State::Guided),
        ),
        Engine::DaemonDown(detail) => Verdict::Fail(
            Problem::new(
                DAEMON_DOWN,
                Severity::Error,
                "Docker is installed but its daemon is not running",
                "The client is here, so this is the daemon being stopped rather than a missing install — starting it is usually all this needs.",
                Remedy::new("Start Docker Desktop, or the docker service on Linux"),
            )
            .in_state(State::Guided)
            .with_detail(detail),
        ),
        Engine::Unusable(reason) => Verdict::Fail(
            Problem::new(
                DAEMON_DOWN,
                Severity::Error,
                "Docker is installed but would not start",
                "The client is present, so this is usually a permission problem rather than a missing install.",
                Remedy::new("Check that you may run docker, then try again"),
            )
            .with_detail(reason),
        ),
    }
}

/// The problem for a Compose plugin that is missing where Docker is present.
fn compose_absent() -> Problem {
    Problem::new(
        COMPOSE_UNUSABLE,
        Severity::Error,
        "The Docker Compose plugin is not available",
        "lemonfiber drives your stack through Compose v2. Docker is here, but its Compose plugin is not, so nothing can be composed.",
        Remedy::new("Install the Docker Compose plugin")
            .with_detail("https://docs.docker.com/compose/install/"),
    )
    .in_state(State::Guided)
}

/// The problem for a Compose plugin older than the floor.
fn compose_outdated(found: &str) -> Problem {
    let (major, minor, patch) = MINIMUM_COMPOSE;
    Problem::new(
        COMPOSE_UNUSABLE,
        Severity::Error,
        format!("Docker Compose {found} is too old to drive this stack"),
        "The commands lemonfiber builds are Compose v2's; an older plugin would not understand them.",
        Remedy::new(format!("Update the Docker Compose plugin to {major}.{minor}.{patch} or newer"))
            .with_detail("https://docs.docker.com/compose/install/"),
    )
    .in_state(State::Guided)
}

/// A finding in the environment category.
fn finding(check: &str, title: &str, verdict: Verdict) -> Finding {
    Finding::in_category(Category::Environment, check, title, verdict)
}

/// A check that ran but could not settle its question, with what to try.
fn unverified(reason: &str) -> Verdict {
    Verdict::Unverified {
        reason: reason.to_owned(),
        remedy: Remedy::new("Run it again once Docker is responding"),
    }
}

/// The engine's own words about why it could not be reached, or a plain
/// statement where it said nothing.
fn reason(stderr: &str) -> String {
    let said = stderr.trim();
    if said.is_empty() {
        "the daemon did not answer".to_owned()
    } else {
        said.to_owned()
    }
}

/// A version, read from a plugin's own report of it.
///
/// Only the leading `major.minor.patch` is significant; a `v` prefix and any
/// build suffix are discarded, because the comparison is against a floor and
/// nothing below the patch changes which side of it a version sits on.
fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let trimmed = text.trim().trim_start_matches('v');
    let mut parts = trimmed.split(['.', '-', '+']);
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

/// A parsed version, back as the string a person reads.
fn describe((major, minor, patch): (u32, u32, u32)) -> String {
    format!("{major}.{minor}.{patch}")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::{
        parse_version, Category, Check, EnvironmentCheck, Verdict, COMPOSE_UNUSABLE, DAEMON_DOWN,
        DOCKER_ABSENT,
    };
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
        ));
        assert!(matches!(
            verdict(&findings, "environment.compose"),
            Some(Verdict::Skipped { reason }) if reason.contains("would not start")
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

    #[test]
    fn a_version_is_read_leniently_and_compared_on_what_matters() {
        assert_eq!(parse_version("v2.32.1\n"), Some((2, 32, 1)));
        assert_eq!(parse_version("2.5"), Some((2, 5, 0)));
        assert_eq!(parse_version("27"), Some((27, 0, 0)));
        assert_eq!(parse_version("2.6.1-desktop.1"), Some((2, 6, 1)));
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("nonsense"), None);
    }
}
