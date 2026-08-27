//! The boundary lemonfiber speaks to the outside world across, and the vocabulary that
//! crosses it.
//!
//! Nothing here talks to anything. These are the seams: the Docker daemon, spawned
//! processes, service HTTP APIs and the wall clock, each named as a trait so the logic
//! above them runs in a test with no daemon, no network and no elapsed time. The
//! implementations live in `lemonfiber-core`'s adapters, and they are the only code there
//! that a unit test cannot run.
//!
//! A crate of its own rather than a module, for two reasons that turned out to be one. The
//! architecture says the boundary is the stable part and the logic above it is not, and a
//! crate makes that something the compiler enforces rather than a sentence in a document.
//! And the fakes the tests drive the real code through implement these traits — as a module
//! inside `lemonfiber-core` they were reachable from its own tests or from its integration
//! tests but never both, so the same port ended up faked twice.
//!
//! The value types here are the ones that cross the boundary: a problem an adapter reports,
//! the stage an item has reached, how long a condition has been standing. They are
//! vocabulary rather than logic — a port that could not name what it returns would push the
//! naming into every caller.
//!
//! Module names avoid repeating their trait's name, so call sites read `ports::Engine`
//! rather than `docker::DockerApi`. See `.docs/architecture/ports-and-adapters.md`.

pub mod docker;
pub mod error;
pub mod filesystem;
pub mod http;
pub mod media;
pub mod narration;
pub mod network;
pub mod nntp;
pub mod process;
pub mod random;
pub mod service;
pub mod time;
pub mod trace;
pub mod withheld;

pub use docker::Engine;
pub use filesystem::{FileSystem, Volume};
pub use http::Http;
pub use narration::Narrator;
pub use network::Site;
pub use process::Runner;
pub use random::Random;
pub use service::Client;
pub use time::Clock;
