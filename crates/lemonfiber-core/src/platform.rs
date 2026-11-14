//! Which environment this is, read through the one component that knows.
//!
//! [`lemonfiber_platform`] decides the operating system and the four environments as
//! pure functions over what it is handed. What is left here is handing it the core's
//! own vocabulary: where the engine was found, and which service manager each
//! operating system has.

pub use lemonfiber_platform::host::{Environment, HostOs, HOST_OS};

use crate::ports::docker::{Reach, Target};
use crate::ports::hosting::Manager;

/// Which service manager keeps a long-running command going on `host`.
///
/// A function over the target rather than a test of it, for the reason
/// [`Environment::resolve`] is one: all three answers are reachable from one
/// laptop, so what a report says on a platform this machine is not is proven
/// here rather than hoped for.
#[must_use]
pub const fn manager(host: HostOs) -> Manager {
    match host {
        HostOs::MacOs => Manager::Launchd,
        HostOs::Linux => Manager::Systemd,
        // Windows has a way to start something at login and lemonfiber does not
        // configure it, which is a thing to say rather than a thing to guess at.
        HostOs::Windows | HostOs::Other => Manager::Unsupported,
    }
}

/// Whether the engine `target` names is Docker Desktop's, as
/// [`Environment::engine_is_desktop`] reads where it was found.
#[must_use]
pub fn engine_is_desktop(target: &Target) -> bool {
    let (context, socket) = found(target);
    Environment::engine_is_desktop(context, socket)
}

/// Whether the engine `target` names is `OrbStack`'s, as
/// [`Environment::engine_is_orbstack`] reads where it was found.
#[must_use]
pub fn engine_is_orbstack(target: &Target) -> bool {
    let (context, socket) = found(target);
    Environment::engine_is_orbstack(context, socket)
}

/// The context `target` was found through and the socket it is reached on, where
/// either is one.
fn found(target: &Target) -> (Option<&str>, Option<&str>) {
    let socket = if let Reach::Socket(path) = &target.reach {
        Some(path.as_str())
    } else {
        None
    };
    (target.context(), socket)
}

#[cfg(test)]
mod tests;
