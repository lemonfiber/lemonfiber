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
//! | [`frontend`] | Where the web app's files come from |
//! | [`docker`] | Making sense of what the engine reports |
//! | [`dashboard`] | One screen of what the stack is doing, assembled honestly |
//! | [`config`] | What the operator chose, and where it is kept |
//! | [`doctor`] | Checks that prove things rather than assuming them |
//! | [`prerequisites`] | What the operator must obtain, from what they chose |
//! | [`seed`] | Wiring the services to each other |
//! | [`journal`] | What was changed, so it can be undone |
//! | [`backup`] | Making a configuration recoverable from an archive |
//!
//! Rust-specific detail lives in this repo's `.docs/architecture/`; the product
//! decisions behind it live in the specification.

// Moved below the boundary and re-exported here under the names the rest of this crate
// already uses. The traits and the vocabulary that crosses them are a crate of their own —
// see `lemonfiber-ports` — so the fakes that stand in for them are reachable from this
// crate's own tests and from its integration tests alike.
pub use lemonfiber_ports as ports;
pub use lemonfiber_ports::{error, trace};

pub mod acknowledged;
pub mod adapters;
pub mod admission;
pub mod agreement;
pub mod alert;
pub mod app;
pub mod archive;
pub mod audio;
pub mod audiobookshelf;
pub mod backup;
pub mod baseline;
pub mod bazarr;
pub mod bindery;
pub mod bundle;
pub mod bytes;
pub mod clients;
pub mod condition;
pub mod config;
pub mod contract;
pub mod dashboard;
pub mod docker;
pub mod doctor;
pub mod door;
mod endpoint;
pub mod frontend;
pub mod glossary;
pub mod health;
pub mod household;
mod instant;
pub mod invitation;
pub mod jellyfin;
pub mod journal;
pub mod lidarr;
pub mod logs;
pub mod materialised;
pub mod model;
pub mod notify;
pub mod outbound;
pub mod platform;
pub mod plural;
pub mod prerequisites;
pub mod provider;
pub mod prowlarr;
pub mod qbittorrent;
pub mod quality;
pub mod queue;
pub mod recyclarr;
pub mod repair;
pub mod retry;
pub mod sabnzbd;
pub mod secret;
pub mod seed;
pub mod seerr;
pub mod servarr;
pub mod stack;
pub mod storage;
pub mod stored;
#[cfg(test)]
mod test_support;
pub mod text;
pub mod transcoding;
pub mod validate;
pub mod walkthrough;
pub mod within;
pub mod wizard;

/// The name of the product, as it appears to users.
pub const PRODUCT: &str = "lemonfiber";
