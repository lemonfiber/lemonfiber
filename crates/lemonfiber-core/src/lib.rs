//! Everything lemonfiber does, with no way to show it.
//!
//! This crate has no user-interface dependency of any kind — no terminal, no
//! argument parser, no HTTP server. It cannot print. That makes "surfaces are
//! renderings, never capabilities" structural rather than aspirational: a
//! surface cannot acquire behaviour of its own, because the behaviour lives
//! somewhere that cannot render it.
//!
//! It also means nearly all of this is testable with no terminal and no daemon,
//! which is what makes the test pyramid viable at all.
//!
//! # Finding your way around
//!
//! | Module | Holds |
//! |--------|-------|
//! | [`app`] | The one entry point: a command in, an outcome out |
//! | [`model`] | The values surfaces render, and serialise |
//! | [`ports`] | The traits the outside world is reached through |
//! | [`adapters`] | The only code that talks to it |
//! | [`error`] | What every failure looks like |
//! | [`platform`] | Which of the four environments this is |
//! | [`stack`] | Building and running a slice of the stack |
//! | [`docker`] | Making sense of what the engine reports |
//! | [`config`] | What the operator chose, and where it is kept |
//! | [`doctor`] | Checks that prove things rather than assuming them |
//! | [`seed`] | Wiring the services to each other |
//! | [`journal`] | What was changed, so it can be undone |
//!
//! Rust-specific detail lives in this repo's `.docs/architecture/`; the product
//! decisions behind it live in the specification.

pub mod adapters;
pub mod app;
pub mod config;
pub mod docker;
pub mod doctor;
pub mod error;
pub mod journal;
pub mod model;
pub mod platform;
pub mod ports;
pub mod seed;
pub mod stack;

/// The name of the product, as it appears to users.
pub const PRODUCT: &str = "lemonfiber";
