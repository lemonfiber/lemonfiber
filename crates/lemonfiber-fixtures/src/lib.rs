//! The fakes lemonfiber's tests drive the real code through.
//!
//! One home, reachable from both sides of a wall. A crate's own `#[cfg(test)]` modules and
//! its `tests/` directory are separate compilation units, so a fake defined in either is
//! invisible to the other, and a port faked in each would be faked twice.
//!
//! A crate rather than a module, because that is the only shape both can reach. It speaks
//! the boundary's types and never depends on `lemonfiber-core`: that would be a
//! dev-dependency cycle, and Cargo would build that crate twice, leaving a fake that
//! implements a trait belonging to neither copy the test is using.
//!
//! One fake per port, scripted rather than subclassed. A test says what a service answers;
//! it does not write another service. A request nothing answered is unreachable rather than
//! a helpful default — a test that reaches an endpoint it did not script has found
//! something, and quietly handing it a `200` would hide it.
//!
//! Every test double of a port lives here; an architecture test refuses one written
//! anywhere else.

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

pub mod applications;
pub mod attestations;
pub mod collecting;
pub mod confined;
pub mod declaring;
pub mod dialled;
pub mod erasing;
pub mod files;
pub mod filing;
pub mod gating;
pub mod git;
pub mod guarded;
pub mod heard;
pub mod hosting;
pub mod household;
pub mod http;
pub mod identity;
pub mod keying;
pub mod located;
pub mod machine;
pub mod mounted;
pub mod networked;
pub mod owning;
pub mod passwords;
pub mod ports;
pub mod probing;
pub mod program;
pub mod providing;
pub mod proving;
pub mod pulled;
pub mod receiving;
pub mod registered;
pub mod remembering;
pub mod scratch;
pub mod serving;
pub mod shelved;
pub mod stack;
pub mod staged;
pub mod support;
pub mod tunnelled;
pub mod typing;
pub mod upstream;
pub mod volumed;
pub mod walking;
