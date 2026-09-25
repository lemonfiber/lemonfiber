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
use serde::Serialize;
use thiserror::Error;
use tokio::sync::mpsc::Receiver;

use lemonfiber_error::codes::docker::{
    ENDPOINT_UNSUPPORTED, ENGINE_UNREACHABLE, HOST_REFUSED, HOST_SILENT, HOST_UNRESOLVED,
    LOGIN_REJECTED, NO_SUCH_CONTAINER, UNKNOWN_CONTEXT,
};
use lemonfiber_error::{Diagnose, Problem, Remedy, Severity, State};

mod locations;
mod target;

pub use locations::{Locations, Presence};
pub use target::{chosen, Choice, Origin, Reach, Target, DEFAULT_CONTEXT};

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

/// One port a container actually answers on, as the engine reports it.
///
/// The engine's own account of what it published, not the file that asked for it.
/// A mapping edited by hand and applied, an image whose defaults changed under an
/// upgrade, a container started outside Compose — all of them arrive here and none
/// of them arrives in a compose file, which is the whole reason a check reads this
/// rather than that.
///
/// One entry per address, so a port published on both families is two of these and a
/// policy can be read on each of them separately. A port a container exposes and
/// nothing published has no host address at all and is not one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Published {
    /// The host address it answers on.
    pub address: std::net::IpAddr,
    /// The port on the host, which is the one an operator types.
    pub port: u16,
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
    /// Every host address and port it actually answers on.
    ///
    /// Read from the engine rather than from what asked for it, so a check about
    /// where things listen is asking what is listening.
    pub published: Vec<Published>,
    /// Every host path mounted into it.
    ///
    /// Where the container's data actually lives on this machine, which is the only
    /// way to tell whether an existing setup keeps its downloads and its library
    /// somewhere a hardlink can reach between. Read from the engine rather than from
    /// a compose file, because what is mounted is what the container got.
    pub mounts: Vec<std::path::PathBuf>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// One line of output from one service.
///
/// Serialisable because a log stream is part of the machine-readable contract:
/// `--json` renders one envelope per line, since a stream has no last element
/// to close a document with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
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
    /// How many lines a read that was told nothing begins with.
    ///
    /// Fifty: enough to hold the thing that went wrong and its lead-up, short
    /// enough to read. Here rather than on each surface because a caller that
    /// said nothing about it said nothing on either — the command line and the
    /// HTTP read must answer the same silence with the same lines, and each of
    /// them saying so in a comment beside its own copy of the number is a
    /// promise nothing was keeping.
    pub const BEGIN_WITH: u32 = 50;

    /// How many lines are quoted from a service that has gone wrong.
    ///
    /// Twenty: the last words, not the history. One question asked in two
    /// places — of a service that would not start, and of a service a
    /// walkthrough was watching — and the same answer is owed to both, because
    /// what an operator is shown of a failure should not depend on which of
    /// them noticed it.
    pub(crate) const LAST_WORDS: u32 = 20;

    /// The last `tail` lines, and then nothing more.
    #[must_use]
    pub const fn recent(tail: u32) -> Self {
        Self {
            tail,
            follow: false,
        }
    }

    /// The last words of a service that has gone wrong.
    ///
    /// A call rather than a constant handed to [`Self::recent`], so that neither
    /// caller names a number at all and there is nothing left to retype.
    #[must_use]
    pub const fn last_words() -> Self {
        Self::recent(Self::LAST_WORDS)
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
    /// The host the endpoint names could not be turned into an address.
    #[error("`{host}` could not be found on the network: {reason}")]
    Unresolved {
        /// The host as it may be shown.
        host: String,
        /// The resolver's own words.
        reason: String,
    },
    /// The host was found and would not accept a connection.
    #[error("`{host}` refused the connection: {reason}")]
    Refused {
        /// The host as it may be shown.
        host: String,
        /// The transport's own words.
        reason: String,
    },
    /// The host was reached and would not accept the login.
    #[error("`{host}` did not accept the SSH login: {reason}")]
    Rejected {
        /// The host as it may be shown.
        host: String,
        /// The client's own words.
        reason: String,
    },
    /// The endpoint names a transport this build cannot drive.
    #[error("`{endpoint}` is not an endpoint lemonfiber can reach")]
    Unsupported {
        /// The endpoint as it may be shown.
        endpoint: String,
    },
    /// A Docker context was named and this machine records no such context.
    #[error("no Docker context named `{name}` is recorded on this machine")]
    NoSuchContext {
        /// The context that was asked for.
        name: String,
    },
    /// The remote host did not answer, in a way none of the three above describes.
    ///
    /// Its own variant rather than the local engine's, because the local one's
    /// remedy is to start Docker on this machine — which, for an operator whose
    /// laptop is talking to a server, is running perfectly.
    #[error("`{host}` did not answer: {reason}")]
    Unanswered {
        /// The host as it may be shown.
        host: String,
        /// The transport's own words.
        reason: String,
    },
}

/// What to tell an operator whose engine is not answering at all.
///
/// Its own function because the arm that dispatches has to stay a dispatch: six
/// problems written inline is a function longer than one may be here, and the
/// distinctions between them are the whole point of there being six.
fn down(reason: &str) -> Problem {
    Problem::new(
        ENGINE_UNREACHABLE,
        Severity::Error,
        "The container engine is not running",
        "Nothing about your stack can be read or changed while the engine is down, so this is the first thing to fix.",
        Remedy::new("Start Docker Desktop, or the docker service on Linux"),
    )
    .in_state(State::Guided)
    .with_detail(reason.to_owned())
}

/// What to tell an operator who asked about a container that is not there.
fn absent(name: &str) -> Problem {
    Problem::new(
        NO_SUCH_CONTAINER,
        Severity::Warning,
        format!("{name} is not running"),
        "The service was expected to be up. It may have stopped on its own, or never been started.",
        Remedy::new("Start the form that includes it").with_detail("lemonfiber ps"),
    )
}

/// What to tell an operator whose host name went nowhere.
///
/// Kept apart from the refusal beside it because the two are different problems
/// with different remedies, and folding them together is how both come to be
/// reported as the local Docker Desktop being stopped — which is wrong for each of
/// them and unactionable for both.
fn unresolved(host: &str, reason: &str) -> Problem {
    Problem::new(
        HOST_UNRESOLVED,
        Severity::Error,
        format!("{host} could not be found on the network"),
        "The name was looked up and nothing answered to it, so no connection was attempted. This is a name problem rather than a Docker one: the daemon may be running perfectly on a machine this one cannot name.",
        Remedy::new("Check the host name, and that this machine can resolve it")
            .with_detail("Try the host's address in place of its name"),
    )
    .in_state(State::Guided)
    .with_detail(reason.to_owned())
}

/// What to tell an operator whose host answered by declining.
fn declined(host: &str, reason: &str) -> Problem {
    Problem::new(
        HOST_REFUSED,
        Severity::Error,
        format!("{host} refused the connection"),
        "The machine was found and answered by declining, so the name is right and something about the endpoint is not. Either Docker is not running over there, or it is not listening where this endpoint says it is.",
        Remedy::new("Check Docker is running on that machine, and on the port the endpoint names"),
    )
    .in_state(State::Guided)
    .with_detail(reason.to_owned())
}

/// What to tell an operator whose key the other machine would not take.
fn rejected(host: &str, reason: &str) -> Problem {
    Problem::new(
        LOGIN_REJECTED,
        Severity::Error,
        format!("{host} did not accept the SSH login"),
        "The machine was reached and the login was refused, so this is about keys and accounts rather than about Docker. lemonfiber uses the SSH configuration you already have and makes no keys of its own.",
        Remedy::new("Check the login works on its own, then try again")
            .with_detail("Connect to the host with ssh and read what it says"),
    )
    .in_state(State::Guided)
    .with_detail(reason.to_owned())
}

/// What to tell an operator whose endpoint names a transport this cannot drive.
///
/// Refused rather than attempted, and the reason is the whole point of this type:
/// an endpoint the reads cannot use is one the writes must not use either, or the
/// operator is shown one machine and changes another.
fn unsupported(endpoint: &str) -> Problem {
    Problem::new(
        ENDPOINT_UNSUPPORTED,
        Severity::Error,
        format!("{endpoint} is not an endpoint lemonfiber can reach"),
        "lemonfiber drives the engine over a local socket, over plain TCP, or over SSH. Nothing was read and nothing was changed, because reading one machine while writing to another is worse than not reaching either.",
        Remedy::new("Point DOCKER_HOST at an ssh:// or tcp:// endpoint, or at a local socket"),
    )
    .in_state(State::Guided)
}

/// What to tell an operator whose context this machine has never heard of.
///
/// Refused rather than quietly answered with the local daemon. A context name is
/// usually a typo when it is wrong, and the fall back Docker's own tooling does not
/// make would hand an operator who meant the server a report about their laptop.
fn unknown(name: &str) -> Problem {
    Problem::new(
        UNKNOWN_CONTEXT,
        Severity::Error,
        format!("there is no Docker context named {name} on this machine"),
        "A context was named and this machine records no endpoint under that name. Nothing was read and nothing was changed, because falling back to the local daemon would answer about this machine while you were asking about another one.",
        Remedy::new("List the contexts this machine has, and name one of those")
            .with_detail("docker context ls"),
    )
    .in_state(State::Guided)
}

/// What to tell an operator whose remote host said nothing this can read.
///
/// The honest end of the list. It names the host and quotes the transport, which
/// is everything that is actually known, and it does not send somebody to start a
/// Docker that is already running on the wrong machine.
fn silent(host: &str, reason: &str) -> Problem {
    Problem::new(
        HOST_SILENT,
        Severity::Error,
        format!("{host} did not answer"),
        "The endpoint was reached for and nothing usable came back. What the transport said is below; it is the most specific thing known about this, and it names the machine rather than this one.",
        Remedy::new("Check the other machine is up and its Docker is running")
            .with_detail("docker --host <endpoint> version"),
    )
    .in_state(State::Guided)
    .with_detail(reason.to_owned())
}

impl Diagnose for Failure {
    fn problem(&self) -> Problem {
        match self {
            Self::Unreachable { reason } => down(reason),
            Self::NoSuchContainer { name } => absent(name),
            Self::Unresolved { host, reason } => unresolved(host, reason),
            Self::Refused { host, reason } => declined(host, reason),
            Self::Rejected { host, reason } => rejected(host, reason),
            Self::Unsupported { endpoint } => unsupported(endpoint),
            Self::NoSuchContext { name } => unknown(name),
            Self::Unanswered { host, reason } => silent(host, reason),
        }
    }
}

/// One image this machine has pulled, and what is standing on it.
///
/// The projects travel with it because whether an image may be removed is not a
/// property of the image: one that another Compose project's container is built on
/// belongs to that project as much as to this one, and nothing downstream can
/// establish that from a name and a size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Every name it answers to, as the engine reports them.
    ///
    /// Empty for an image nothing tags any more, which is still an image taking up
    /// room and so still worth reporting.
    pub tags: Vec<String>,
    /// What it occupies, as the engine reports its size.
    pub bytes: u64,
    /// The Compose projects whose containers are built on it.
    ///
    /// A container the engine reports under no Compose project contributes an empty
    /// entry, because for the purposes of removal "something outside Compose is
    /// standing on this" is the same answer as "another project is".
    pub projects: Vec<String>,
}

/// Listing the images an engine has pulled, and who is standing on each.
///
/// A trait of its own rather than another method on [`Engine`], for the reason the
/// volume watch and the eraser are apart from the filesystem: the one command that
/// asks needs nothing else of an engine, and every other implementation of the wider
/// trait — five of them are test fakes — would gain a method it never calls.
#[async_trait]
pub trait Images: Send + Sync {
    /// Every image on this machine, with the projects holding containers on it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached, which the
    /// caller reports as an unknown rather than as no images.
    async fn images(&self) -> Result<Vec<Image>, Failure>;
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
    /// Naming `services` narrows to those; naming none takes them all.
    /// Narrowing here rather than at the reader means a stream is never opened
    /// for output nobody asked for.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Unreachable`] when the engine cannot be reached.
    async fn logs(
        &self,
        project: &str,
        services: &[String],
        query: LogQuery,
    ) -> Result<Receiver<LogLine>, Failure>;
}

#[cfg(test)]
mod tests;
