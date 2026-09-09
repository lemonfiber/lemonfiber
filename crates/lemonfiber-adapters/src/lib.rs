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

/// Every seam, wired to the real machine.
///
/// The one place the real implementations are chosen, so a surface does not name ten of
/// them and a core cannot name any. What a caller varies afterwards it varies by name,
/// on the context, which is what keeps a fake visible at the call that installs it.
///
/// Takes nothing. Everything here reaches the machine on its own account; a seam built
/// over another seam — asking this machine its name by running a program — is written
/// over the port rather than over the machine, and stayed in the core with the rest of
/// the composition.
#[must_use]
pub fn live() -> Seams {
    Seams {
        filesystem: Arc::new(Disk),
        // Wrapped so a service that is merely still starting is tried again rather than
        // reported. Applied here rather than at each caller: a retry policy written into
        // fifteen call sites is fifteen policies.
        http: Arc::new(Retrying::around(Web::new())),
        images: Arc::new(Daemon::local()),
        volume: Arc::new(Disk),
        eraser: Arc::new(Disk),
        occupancy: Arc::new(Disk),
        hosting: Arc::new(Unhosted),
        random: Arc::new(Os),
        nntp: Arc::new(Dialer::new()),
    }
}
