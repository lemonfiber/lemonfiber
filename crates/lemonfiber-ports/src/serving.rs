//! Serving an image as its service runs, in a container of its own on a network of its
//! own, so that it can be asked over the loopback interface and then taken away.
//!
//! Every container this starts is new, from the image the caller names by digest, never
//! a pulled image, on a network nothing else joins, with its one port published on the
//! loopback interface alone, and removed with its network whatever it came to.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::program::Unran;

/// One service, as the caller settled it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Serve {
    /// The image, as `image@sha256:…`.
    pub image: String,
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
    /// Each device on the engine's machine passed into it, as `host:container`.
    pub devices: Vec<String>,
    /// Each directory on the engine's machine mounted into it.
    pub binds: Vec<Bound>,
    /// Each scratch path it holds in memory.
    pub tmpfs: Vec<String>,
    /// The port it answers on inside its network, published on the loopback interface
    /// at a port the engine chooses.
    pub listens: Option<u16>,
    /// The name every other container on its network reaches it by, where it has one.
    pub alias: Option<String>,
    /// Each name it resolves to the machine lemonfiber runs on, where a stand-in answers.
    pub reaching: Vec<String>,
}

/// What lemonfiber answers itself while it serves, at a port on its own machine that the
/// containers it serves reach by a name in [`Serve::reaching`], and nothing else reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandIn {
    /// The port it answers on.
    pub port: u16,
    /// Each header every answer carries, as a name and its value.
    pub headers: Vec<(String, String)>,
    /// Each question it answers; any other is refused.
    pub answers: Vec<Said>,
}

/// One question a stand-in answers, and what it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// The method it is asked with.
    pub method: String,
    /// The path it is asked at, matched without regard to case.
    pub path: String,
    /// A query parameter it is asked with and the value it holds, where one names what is
    /// asked at that path.
    pub query: Option<(String, String)>,
    /// The status it answers with.
    pub status: u16,
    /// What it answers, as JSON text, where the answer has a body; each `{query:<name>}` in
    /// it is the value the query parameter `<name>` held the last time a request to the
    /// stand-in carried it, and empty where none has.
    pub body: Option<String>,
}

impl Said {
    /// `body`, JSON text, answered with `200` to `method` at `path`.
    #[must_use]
    pub fn json(method: &str, path: &str, body: String) -> Self {
        Self {
            method: method.to_owned(),
            path: path.to_owned(),
            query: None,
            status: 200,
            body: Some(body),
        }
    }

    /// `204` and no body, answered to `method` at `path`.
    #[must_use]
    pub fn empty(method: &str, path: &str) -> Self {
        Self {
            method: method.to_owned(),
            path: path.to_owned(),
            query: None,
            status: 204,
            body: None,
        }
    }
}

/// A stand-in answering, which stops answering when it is dropped.
pub trait Answering: Send + Sync {}

/// A directory on the engine's machine, mounted at a path inside the container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    /// The directory mounted.
    pub source: PathBuf,
    /// Where it lands inside the container.
    pub target: String,
}

/// A service being served.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Served {
    /// The container's id.
    pub container: String,
    /// The network it alone is on.
    pub network: String,
    /// The name it knows itself by.
    pub hostname: String,
    /// Every address it holds on its network.
    pub addresses: Vec<String>,
    /// The name, kernel version and operating system the engine reports for the machine
    /// it runs on.
    pub machine: Vec<String>,
    /// Where the loopback interface reaches the port it listens on, where it has one.
    pub port: Option<u16>,
}

/// How a container stands, as the engine reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Running, with a health check of its own that has not passed yet.
    Starting,
    /// Running, its own health check passing.
    Healthy,
    /// Running, its own health check failing.
    Unhealthy,
    /// Running, with no health check of its own.
    Running,
    /// No longer running, having exited with this status.
    Exited(i64),
}

/// Serves an image as its service runs.
#[async_trait]
pub trait Serving: Send + Sync {
    /// Create a network and a container on it for `serve`, and start it.
    ///
    /// # Errors
    ///
    /// [`Unran`] where the engine cannot be reached, will not say what machine it runs
    /// on, does not hold the image, or refuses the network or the container; nothing is
    /// left behind in any of them.
    async fn serve(&self, serve: &Serve) -> Result<Served, Unran>;

    /// Create a container for `serve` on the network `served` is on, and start it.
    ///
    /// # Errors
    ///
    /// [`Unran`] where the engine cannot be reached, does not hold the image, or refuses
    /// the container; nothing is left behind in any of the three.
    async fn beside(&self, served: &Served, serve: &Serve) -> Result<Served, Unran>;

    /// How the container stands now.
    ///
    /// # Errors
    ///
    /// [`Unran`] where the engine cannot be reached or no longer holds the container.
    async fn standing(&self, served: &Served) -> Result<Standing, Unran>;

    /// The last `lines` lines it wrote, standard output and standard error together,
    /// or nothing where the engine will not say.
    async fn log(&self, served: &Served, lines: usize) -> String;

    /// Take the container off the engine, and its network once nothing else is on it.
    async fn removed(&self, served: &Served);

    /// Answer `stand_in` on this machine, at the one address the containers this serves
    /// reach it by, until what this returns is dropped.
    ///
    /// # Errors
    ///
    /// [`Unran`] where the engine cannot be reached or will not say where its containers
    /// reach this machine, or the port there cannot be taken.
    async fn stand_in(&self, stand_in: &StandIn) -> Result<Box<dyn Answering>, Unran>;
}
