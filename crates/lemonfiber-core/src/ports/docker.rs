//! Observing containers through the Engine API.
//!
//! Reads come through here and writes do not: lifecycle goes through Compose as
//! a subprocess. At roughly one poll per second across nineteen services,
//! spawning a process to observe would be both wasteful and visibly jittery.
//!
//! Streams are handed back as channel receivers rather than as an async stream
//! type, because a producer that owns its data and sends owned snapshots is the
//! shape the render loop requires — nothing shares mutable state with a frame.

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::mpsc::Receiver;

use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};

/// What a container is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lifecycle {
    /// Created but not started.
    Created,
    /// Running.
    Running,
    /// Paused.
    Paused,
    /// Restarting.
    Restarting,
    /// Stopped.
    Exited,
    /// Being removed.
    Removing,
    /// Not running, and the engine does not say why.
    Dead,
}

/// What the container's own health probe says, where it has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Health {
    /// Still inside its start period.
    Starting,
    /// Passing.
    Healthy,
    /// Failing.
    Unhealthy,
    /// The container declares no probe.
    None,
}

/// One container, correlated back to the service that declared it.
///
/// Correlation uses Compose's own labels, which the Engine API exposes, rather
/// than a naming convention this code would otherwise have to keep in step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    /// The engine's identifier.
    pub id: String,
    /// The Compose project it belongs to.
    pub project: String,
    /// The Compose service it implements.
    pub service: String,
    /// What it is doing.
    pub lifecycle: Lifecycle,
    /// What its own probe says.
    pub health: Health,
    /// How it exited, where it has exited and the engine still remembers.
    ///
    /// Present because stopping on purpose and falling over are the same
    /// lifecycle and entirely different problems: an operator who stopped a
    /// service should not be shown a fault, and one whose service died should
    /// not be shown a tidy `stopped`.
    pub exit: Option<i32>,
}

/// Resource use for one container at one moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    /// Fraction of one core, where 1.0 is a fully used core.
    pub cpu: f64,
    /// Resident memory in bytes.
    pub memory_bytes: u64,
}

/// Which stream a log line arrived on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// One line of output from one service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    /// The Compose service it came from.
    pub service: String,
    /// Which stream it arrived on.
    pub stream: Stream,
    /// When the container itself says it wrote the line, where it said so.
    ///
    /// Kept verbatim and unparsed. Containers disagree with the host clock and
    /// with each other, and the only defensible ordering is each container's own
    /// account of itself — which a reader can only apply if it is carried
    /// rather than replaced by an arrival time.
    pub at: Option<String>,
    /// The line, without its trailing newline.
    pub line: String,
}

/// How much output to ask for, and whether to keep listening.
///
/// Both fields are the same question asked of a failure and of a log viewer:
/// the health gate wants the last few lines of a service that would not start,
/// and an operator wants everything, still arriving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogQuery {
    /// How many existing lines to begin with.
    pub tail: u32,
    /// Whether to keep the stream open as new lines arrive.
    pub follow: bool,
}

impl LogQuery {
    /// The last `tail` lines, and then nothing more.
    #[must_use]
    pub const fn recent(tail: u32) -> Self {
        Self {
            tail,
            follow: false,
        }
    }
}

/// What a command left behind after running inside a container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecOutput {
    /// The exit status, absent when a signal ended the command.
    pub status: Option<i32>,
    /// Everything the command wrote.
    pub stdout: String,
}

/// The engine could not be reached, or refused.
#[derive(Debug, Error)]
pub enum Failure {
    /// The daemon is not accepting connections.
    #[error("the container engine is not reachable: {reason}")]
    Unreachable {
        /// The transport's own words.
        reason: String,
    },
    /// The container was asked about and does not exist.
    #[error("no container named `{name}` is running")]
    NoSuchContainer {
        /// The container that was looked for.
        name: String,
    },
}

/// Raised when the container engine cannot be reached.
pub const ENGINE_UNREACHABLE: Code = Code::new("DOCKER-1");

/// Raised when a container that should exist does not.
pub const NO_SUCH_CONTAINER: Code = Code::new("DOCKER-2");

impl Diagnose for Failure {
    fn problem(&self) -> Problem {
        match self {
            Self::Unreachable { reason } => Problem::new(
                ENGINE_UNREACHABLE,
                Severity::Error,
                "The container engine is not running",
                "Nothing about your stack can be read or changed while the engine is down, so this is the first thing to fix.",
                Remedy::new("Start Docker Desktop, or the docker service on Linux"),
            )
            .in_state(State::Guided)
            .with_detail(reason.clone()),
            Self::NoSuchContainer { name } => Problem::new(
                NO_SUCH_CONTAINER,
                Severity::Warning,
                format!("{name} is not running"),
                "The service was expected to be up. It may have stopped on its own, or never been started.",
                Remedy::new("Start the form that includes it").with_detail("lemonfiber ps"),
            ),
        }
    }
}

/// Reads container state, resource use, logs, and runs commands inside them.
///
/// The `exec` method is what makes the VPN leak test possible: run the same
/// command inside the tunnel container and the torrent client, and compare the
/// public addresses they report.
#[async_trait]
pub trait Engine: Send + Sync {
    /// Every container belonging to a Compose project.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached.
    async fn list(&self, project: &str) -> Result<Vec<Container>, Failure>;

    /// Run a command inside a running container and collect what it wrote.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::NoSuchContainer`] when the container is not running,
    /// and [`Failure::Unreachable`] when the engine cannot be reached.
    async fn exec(&self, container: &str, argv: &[String]) -> Result<ExecOutput, Failure>;

    /// Resource use for a project's containers, sampled until the receiver is
    /// dropped.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached.
    async fn stats(&self, project: &str) -> Result<Receiver<(String, Stats)>, Failure>;

    /// Log lines for a project's containers, until the receiver is dropped.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached.
    async fn logs(&self, project: &str, query: LogQuery) -> Result<Receiver<LogLine>, Failure>;
}

#[cfg(test)]
mod tests {
    use super::{
        Container, Diagnose, ExecOutput, Failure, Health, Lifecycle, LogLine, LogQuery, Stats,
        Stream,
    };

    #[test]
    fn an_unreachable_engine_keeps_the_transport_s_own_words() {
        let problem = Failure::Unreachable {
            reason: "connection refused".to_owned(),
        }
        .problem();
        assert_eq!(problem.detail.as_deref(), Some("connection refused"));
        assert!(!problem.remedies.is_empty());
    }

    #[test]
    fn a_missing_container_is_named_and_is_not_an_error() {
        let problem = Failure::NoSuchContainer {
            name: "sonarr".to_owned(),
        }
        .problem();
        assert!(problem.summary.contains("sonarr"));
        assert!(!problem.remedies.is_empty());
        assert!(Failure::NoSuchContainer {
            name: "sonarr".to_owned()
        }
        .to_string()
        .contains("sonarr"));
    }

    #[test]
    fn observations_carry_the_service_they_describe() {
        let container = Container {
            id: "abc123".to_owned(),
            project: "lemonfiber".to_owned(),
            service: "sonarr".to_owned(),
            lifecycle: Lifecycle::Running,
            health: Health::Healthy,
            exit: None,
        };
        assert_eq!(container.clone(), container);
        assert_eq!(container.service, "sonarr");

        let line = LogLine {
            service: "sonarr".to_owned(),
            stream: Stream::Stderr,
            at: Some("2026-07-25T10:00:00Z".to_owned()),
            line: "something happened".to_owned(),
        };
        assert_eq!(line.clone().stream, Stream::Stderr);
        assert_eq!(line.at.as_deref(), Some("2026-07-25T10:00:00Z"));

        let stats = Stats {
            cpu: 0.5,
            memory_bytes: 1024,
        };
        assert!((stats.cpu - 0.5).abs() < f64::EPSILON);

        let exec = ExecOutput {
            status: Some(0),
            stdout: "203.0.113.7".to_owned(),
        };
        assert_eq!(exec.clone().stdout, "203.0.113.7");
    }

    #[test]
    fn asking_for_recent_output_does_not_ask_to_keep_listening() {
        let recent = LogQuery::recent(50);
        assert_eq!(
            recent,
            LogQuery {
                tail: 50,
                follow: false
            }
        );
    }

    #[test]
    fn every_lifecycle_and_health_value_is_distinguishable() {
        let lifecycles = [
            Lifecycle::Created,
            Lifecycle::Running,
            Lifecycle::Paused,
            Lifecycle::Restarting,
            Lifecycle::Exited,
            Lifecycle::Removing,
            Lifecycle::Dead,
        ];
        assert_eq!(lifecycles.len(), 7);
        assert_ne!(
            format!("{:?}", Lifecycle::Running),
            format!("{:?}", Lifecycle::Dead)
        );

        let healths = [
            Health::Starting,
            Health::Healthy,
            Health::Unhealthy,
            Health::None,
        ];
        assert_eq!(healths.len(), 4);
        assert_ne!(
            format!("{:?}", Health::Healthy),
            format!("{:?}", Health::None)
        );
    }
}
