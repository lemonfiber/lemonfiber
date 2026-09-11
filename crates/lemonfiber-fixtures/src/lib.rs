//! The fakes lemonfiber's tests drive the real code through.
//!
//! One home, reachable from both sides of a wall that used to have no door. A crate's own
//! `#[cfg(test)]` modules and its `tests/` directory are separate compilation units, so a
//! fake defined in either is invisible to the other — which is how the same port came to be
//! faked twice, and the filesystem four times.
//!
//! A crate rather than a module, because that is the only shape both can reach. It depends
//! on `lemonfiber-ports` alone: depending on `lemonfiber-core` would be a dev-dependency
//! cycle, and Cargo would build that crate twice, leaving a fake that implements a trait
//! belonging to neither copy the test is using.
//!
//! One fake per port, scripted rather than subclassed. A test says what a service answers;
//! it does not write another service. A request nothing answered is unreachable rather than
//! a helpful default — a test that reaches an endpoint it did not script has found
//! something, and quietly handing it a `200` would hide it.
//!
//! What is *not* here, and why. The fakes that speak lemonfiber's own models rather than a
//! port — the Servarr service and the VPN gateway — need the seeding and diagnosis models to
//! say anything, so they stay beside the integration tests that use them. And the two that
//! drive the retry wrapper stay with it: `Retrying<H>` takes a concrete transport by value,
//! so wrapping a shared `Arc<Fake>` would mean widening a production signature to suit a
//! test, and what they are for — counting how many times a blip was retried — is the thing
//! under test rather than a service standing in for another.

/// Record one thing a fake was asked to do, where the lock is still good.
///
/// A poisoned lock means a test already failed somewhere else and is unwinding; a
/// second failure raised here would point at the recording rather than at the fault,
/// so the record is dropped instead. Shared because every fake that remembers what it
/// was asked keeps it this way, and a branch repeated eight times is eight places for
/// it to be written differently.
pub(crate) fn noted<T>(into: &std::sync::Mutex<Vec<T>>, what: T) {
    if let Ok(mut held) = into.lock() {
        held.push(what);
    }
}

pub mod downloads;
pub mod erasing;
pub mod files;
pub mod hosting;
pub mod http;
pub mod ports;
pub mod program;
pub mod pulled;
pub mod support;
pub mod walking;
