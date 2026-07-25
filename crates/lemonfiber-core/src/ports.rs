//! The traits everything outside this process is reached through.
//!
//! Nothing here talks to anything. These are the seams: the Docker daemon,
//! spawned processes, service HTTP APIs and the wall clock, each named as a
//! trait so the logic above them runs in a test with no daemon, no network and
//! no elapsed time. The implementations live in [`crate::adapters`], and they
//! are the only code in this crate that a unit test cannot run.
//!
//! Module names avoid repeating their trait's name, so call sites read
//! `ports::Engine` rather than `docker::DockerApi`. See
//! `.docs/architecture/ports-and-adapters.md`.

pub mod docker;
pub mod process;
pub mod service;
pub mod time;

pub use docker::Engine;
pub use process::Runner;
pub use service::Client;
pub use time::Clock;
