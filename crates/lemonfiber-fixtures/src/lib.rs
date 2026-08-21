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

pub mod downloads;
pub mod files;
pub mod http;
pub mod ports;
pub mod support;
