//! The implementations that actually touch the outside world.
//!
//! This is the code a unit test cannot run, which is why it is kept thin:
//! translation, and no decisions. Anything that can be wrong belongs on the other
//! side of a port, where a fake can drive it.
//!
//! It depends on `lemonfiber-ports` and on nothing else of this workspace's. A port
//! is the whole of what an implementation needs to know, and the restraint buys
//! something the rest of the product could not otherwise have: the core cannot reach
//! the network, because it does not depend on anything that can. That used to be a
//! rule a test looked for by reading source text; it is now a fact about the crate
//! graph, and the core's own manifest no longer carries a container runtime, an HTTP
//! client or a TLS stack.
//!
//! The core holds these as trait objects it is handed rather than ones it builds, so
//! nothing above this line can manufacture a socket.
//!
//! An architecture test still holds the narrower half of the line — each external
//! crate is permitted in exactly one file here, so a subsystem cannot quietly grow
//! its own way out. See `.docs/architecture/ports-and-adapters.md`.

pub mod docker;
pub mod filesystem;
pub mod hosting;
pub mod http;
pub mod nntp;
pub mod occupancy;
pub mod process;
pub mod random;
pub mod retrying;
pub mod time;

pub use docker::context::from_environment as docker_target;
pub use docker::Daemon;
pub use filesystem::Disk;
pub use hosting::launchd::Launchd;
pub use hosting::systemd::Systemd;
pub use hosting::Unhosted;
pub use http::Web;
pub use nntp::Dialer;
pub use process::Local;
pub use random::Os;
pub use retrying::Retrying;
pub use time::System;

use std::sync::Arc;

use lemonfiber_ports::seams::Seams;

/// Every seam, wired to the real machine and to this machine's own engine.
///
/// The one place the real implementations are chosen, so a surface does not name ten of
/// them and a core cannot name any. What a caller varies afterwards it varies by name,
/// on the context, which is what keeps a fake visible at the call that installs it.
///
/// Everything here reaches the machine on its own account; a seam built over another
/// seam — asking this machine its name by running a program — is written over the port
/// rather than over the machine, and stayed in the core with the rest of the composition.
#[must_use]
pub fn live() -> Seams {
    live_reaching(&lemonfiber_ports::docker::Target::local())
}

/// Every seam, with the engine ones pointed at the engine this run operates.
///
/// The image listing is an engine read like any other, and it went through a client of
/// its own built from this machine's defaults. On a remote context that put one more
/// reader on the laptop while everything else was on the server — the same split this
/// version exists to close, in the one seam nobody thinks of as an engine.
///
/// Takes the target rather than resolving one, so there is one answer for the run and
/// no seam can be built against a second.
#[must_use]
pub fn live_reaching(target: &lemonfiber_ports::docker::Target) -> Seams {
    Seams {
        filesystem: Arc::new(Disk),
        // Wrapped so a service that is merely still starting is tried again rather than
        // reported. Applied here rather than at each caller: a retry policy written into
        // fifteen call sites is fifteen policies.
        http: Arc::new(Retrying::around(Web::new())),
        images: Arc::new(Daemon::reaching(target.clone())),
        // The third seam built from the one resolved target, and the reason it is
        // built from it rather than from this machine's defaults is the whole of what
        // it is for: a pre-flight that asked the laptop whether the server has a
        // directory would answer yes and be wrong in exactly the case it exists for.
        locations: Arc::new(Daemon::reaching(target.clone())),
        volume: Arc::new(Disk),
        eraser: Arc::new(Disk),
        occupancy: Arc::new(Disk),
        hosting: Arc::new(Unhosted),
        random: Arc::new(Os),
        nntp: Arc::new(Dialer::new()),
    }
}
