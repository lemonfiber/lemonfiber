//! The implementations that actually touch the outside world.
//!
//! This is the only code in the crate a unit test cannot run, which is why it is
//! kept thin: translation, and no decisions. Anything that can be wrong belongs
//! on the other side of a port, where a fake can drive it.
//!
//! An architecture test holds the line — each external crate is permitted in
//! exactly one place, so a subsystem cannot quietly grow its own way out.
//!
//! The Docker and service-API adapters arrive with the milestones that need
//! them; their ports are defined so the logic above can be written and tested
//! first. See `.docs/architecture/ports-and-adapters.md`.

pub mod process;
pub mod time;

pub use process::Local;
pub use time::System;
