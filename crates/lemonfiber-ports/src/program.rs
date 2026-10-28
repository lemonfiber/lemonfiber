//! Running an image's own program once, in a container of its own.
//!
//! Every container this starts is new, from the image the caller names by digest, with
//! no network, no mount but the one directory it is handed, never a pulled image, and
//! removed when the run ends whatever it came to.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;

/// How old what a run leaves on the machine has to be before a later run takes it for
/// left behind: past any run's limit, with room to make and remove its container.
pub const LEFT_BEHIND: Duration = Duration::from_secs(600);

/// One run, as the caller settled it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// The image, as `image@sha256:…`.
    pub image: String,
    /// Each argument handed to the image's entrypoint.
    pub args: Vec<String>,
    /// Its whole environment, as `NAME=value`.
    pub env: Vec<String>,
    /// Who it runs as, as `uid:gid`, or nothing for the image's own choice.
    pub user: Option<String>,
    /// What it may not gain once started.
    pub security_opt: Vec<String>,
    /// The kernel capabilities it starts without.
    pub cap_drop: Vec<String>,
    /// The kernel capabilities it is given back.
    pub cap_add: Vec<String>,
    /// How many processes it may hold.
    pub pids_limit: i64,
    /// Whether its root filesystem is read-only.
    pub read_only: bool,
    /// Its memory ceiling in bytes, where it has one.
    pub memory: Option<i64>,
    /// The directory mounted at its configuration path, where it has one.
    pub configuration: Option<Configuration>,
    /// How long it may take before it is stopped.
    pub within: Duration,
    /// The most of its standard output that is read.
    pub read_at_most: usize,
}

/// A directory on the engine's machine, mounted at a path inside the container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuration {
    /// The directory mounted.
    pub source: PathBuf,
    /// Where it lands inside the container.
    pub target: String,
}

/// What a run came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    /// How it ended.
    pub ended: Ended,
    /// What it wrote to standard output, up to the most the run reads.
    pub stdout: Vec<u8>,
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// It exited with this status.
    Exited(i64),
    /// It was still running at its time limit and was stopped.
    Overran,
}

/// Why a run could not be made.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Unran {
    /// The engine could not be reached.
    #[error("the container engine could not be reached: {reason}")]
    Unreachable {
        /// The transport's own words.
        reason: String,
    },
    /// The engine does not hold the image at the digest asked for, and pulls nothing.
    #[error("the engine does not hold {image}, and a proof pulls no image")]
    NoImage {
        /// The image, as it was asked for.
        image: String,
    },
    /// The engine refused to create, start or watch the container.
    #[error("the container engine refused the run: {reason}")]
    Refused {
        /// The engine's own words.
        reason: String,
    },
    /// The engine did not start the container within the time a start is given.
    #[error("the container engine did not start the container within {seconds} seconds")]
    Unstarted {
        /// How long it was given, in seconds.
        seconds: u64,
    },
}

/// Runs an image's own program once.
#[async_trait]
pub trait Program: Send + Sync {
    /// Create, start, read, wait on and remove one container for `run`.
    ///
    /// # Errors
    ///
    /// [`Unran`] where the engine cannot be reached, does not hold the image, or refuses
    /// the container; nothing is left behind in any of the three.
    async fn run(&self, run: &Run) -> Result<Ran, Unran>;
}
