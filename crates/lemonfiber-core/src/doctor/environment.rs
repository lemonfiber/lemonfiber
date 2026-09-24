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
use crate::error::{Problem, Remedy, Severity, State};
use crate::ports::docker::Target;
use crate::ports::process::Failure;
use crate::ports::Runner;

pub(crate) use crate::error::codes::env::DOCKER_ABSENT;

pub(crate) use crate::error::codes::env::DAEMON_DOWN;

pub(crate) use crate::error::codes::env::COMPOSE_UNUSABLE;

pub(crate) use crate::error::codes::env::API_MISMATCH;

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
    target: Target,
}

/// What the client said when it answered.
///
/// Three numbers from one call rather than one from each of three, because the
/// client connects to answer any of them and a second connection is a second
/// moment — which is how two versions come to be reported from different daemons
/// and compared as though they were not.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Answered {
    /// What the daemon calls its own release.
    server: String,
    /// The API generation this machine's client speaks.
    ours: String,
    /// The API generation the daemon speaks.
    theirs: String,
}

/// What running the Docker client revealed about the engine.
enum Engine {
    /// The client answered and named its daemon's version.
    Up(Answered),
    /// The client is not installed.
    Absent,
    /// The client is present but its daemon did not answer.
    DaemonDown(String),
    /// The client is installed but the operating system would not start it.
    Unusable(String),
}

impl EnvironmentCheck {
    /// A check over the given way of running programs, against this machine's engine.
    #[must_use]
    pub fn new(runner: Arc<dyn Runner>) -> Self {
        Self::reaching(runner, Target::local())
    }

    /// A check over the given way of running programs, against the engine this run
    /// operates.
    ///
    /// The endpoint is named on the invocation rather than left to the environment,
    /// for the reason the Compose invocation names it: a report about the API
    /// generations in play has to be about the daemon the stack is actually run
    /// against, and a client left to work that out for itself is a second opinion
    /// about which machine this is.
    #[must_use]
    pub fn reaching(runner: Arc<dyn Runner>, target: Target) -> Self {
        Self { runner, target }
    }

    /// Ask the Docker client for its daemon's version, and read the answer for
    /// which of the four things it means.
    async fn engine(&self) -> Engine {
        match self.runner.run(&docker_version(&self.target)).await {
            Err(Failure::NotFound { .. }) => Engine::Absent,
            Err(Failure::Unusable { reason, .. }) => Engine::Unusable(reason),
            Ok(output) => {
                let said = answered(&output.stdout);
                if output.succeeded() && !said.server.is_empty() {
                    Engine::Up(said)
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
        ran(self).await
    }
}

/// Whether this machine has what the stack needs to run at all.
async fn ran(check: &EnvironmentCheck) -> Vec<Finding> {
    let engine = check.engine().await;
    let compose = match &engine {
        // With no client there is nothing to run Compose through, so it is
        // skipped with the same reason the operator will act on first.
        Engine::Absent => Verdict::Skipped {
            reason: "Docker is not installed".to_owned(),
        },
        Engine::Unusable(_) => Verdict::Skipped {
            reason: "the Docker client would not start".to_owned(),
        },
        Engine::Up(_) | Engine::DaemonDown(_) => check.compose().await,
    };

    // Read before the engine finding takes the answer, because both are about the
    // same one call: asking the client twice would be asking two daemons on the day
    // it matters.
    let api = api_verdict(&engine);

    vec![
        finding(
            "environment.engine",
            "Docker engine",
            engine_verdict(engine),
        ),
        finding("environment.api", "Docker API version", api),
        finding("environment.compose", "Docker Compose", compose),
    ]
}

/// What the client said, split into the three numbers it was asked for.
///
/// A client that answered with fewer fields than it was asked for — an older one
/// that does not know a template key — leaves the rest empty, which reads as not
/// stated rather than as zero.
fn answered(stdout: &str) -> Answered {
    let mut fields = stdout.trim().split(SEPARATOR);
    let mut next = || fields.next().unwrap_or_default().trim().to_owned();
    Answered {
        server: next(),
        ours: next(),
        theirs: next(),
    }
}

/// Whether the two ends speak the same generation of the engine's API.
///
/// Reported rather than refused. The client and this build both negotiate down to
/// whatever the daemon offers, so a mismatch is a difference in what is available
/// rather than a thing that is broken — and the operator hunting a feature that is
/// not there needs the two numbers in front of them, which is the whole of what
/// this finding is for.
fn api_verdict(engine: &Engine) -> Verdict {
    let Engine::Up(said) = engine else {
        return Verdict::Skipped {
            reason: "the daemon did not answer, so neither version could be read".to_owned(),
        };
    };
    if said.ours.is_empty() || said.theirs.is_empty() {
        return unverified("the Docker client did not report both API versions");
    }
    if said.ours == said.theirs {
        return Verdict::Pass {
            note: Some(said.theirs.clone()),
        };
    }
    Verdict::Warn(mismatch(&said.ours, &said.theirs))
}

/// The problem for two ends that agreed on a generation neither of them prefers.
///
/// Both numbers, always. One of them is useless: an operator told the daemon speaks
/// something older has no idea whether that is one release behind or six, and the
/// pair is the only form of this that can be acted on.
fn mismatch(ours: &str, theirs: &str) -> Problem {
    Problem::new(
        API_MISMATCH,
        Severity::Warning,
        format!("this machine speaks Docker API {ours} and the daemon speaks {theirs}"),
        "Both ends settle on the older of the two, so everything works and anything newer than that generation is simply not available. It is worth knowing about when a feature that should be there is not, and it is ordinary where the two machines were updated at different times.",
        Remedy::new("Bring both to the same Docker release, or carry on with the older set"),
    )
    .in_state(State::Guided)
}

/// What separates the three fields the client is asked for.
///
/// A character no version string carries, so splitting cannot take a version apart.
const SEPARATOR: char = '|';

/// What the client is asked to print: the daemon's release, then the API generation
/// at each end.
const FORMAT: &str = "{{.Server.Version}}|{{.Client.APIVersion}}|{{.Server.APIVersion}}";

/// The command that asks the client about the daemon it fronts.
///
/// The server field is the point: the client prints its own version without a
/// daemon, and only names the server once one has answered. The two API fields ride
/// along because they are the same connection — and the daemon they describe has to
/// be the one the stack is run against, which is why the endpoint is named here.
fn docker_version(target: &Target) -> Vec<String> {
    let mut argv = vec!["docker".to_owned()];
    if let Some(endpoint) = target.endpoint() {
        argv.push("--host".to_owned());
        argv.push(endpoint.to_owned());
    }
    argv.push("version".to_owned());
    argv.push("--format".to_owned());
    argv.push(FORMAT.to_owned());
    argv
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
        Engine::Up(said) => Verdict::Pass {
            note: Some(said.server),
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
            .in_state(State::Guided)
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
mod tests;
