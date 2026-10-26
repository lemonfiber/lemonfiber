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
//! | [`plugin`] | The three artefacts a plugin author reads, generated |
//! | [`filling`] | Which service fills a capability a wiring asks for |
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
pub use lemonfiber_error as error;
pub use lemonfiber_ports as ports;
pub use lemonfiber_ports::service::stage as trace;

pub mod acknowledged;
pub mod admission;
pub mod age_limit;
pub mod agreement;
pub mod alert;
pub mod app;
pub mod app_keys;
pub mod archive;
pub mod asking;
pub mod audio;
pub mod autostart;
pub mod backup;
pub mod bandwidth;
pub mod baseline;
pub mod bundle;
pub mod bytes;
pub mod certificate;
pub mod changelog;
pub mod clients;
pub mod companion;
pub mod condition;
pub mod config;
pub mod contained;
pub mod credential;
pub mod dashboard;
pub mod decline;
pub mod device_keys;
pub mod docker;
pub mod doctor;
pub mod door;
mod endpoint;
pub mod filling;
pub mod frontend;
pub mod generated;
pub mod glossary;
pub mod health;
pub mod household;
pub mod instant;
pub mod invitation;
pub mod journal;
pub mod keys;
pub mod logs;
pub mod machine;
pub mod materialised;
pub mod migration;
pub mod model;
pub mod notify;
pub mod origin;
pub mod outbound;
pub mod outward;
pub mod overlay;
pub mod pass;
pub(crate) mod patience;
pub mod platform;
pub use crate::error::plural;
pub mod network;
pub mod news;
pub mod plugin;
pub mod prerequisites;
pub mod project;
pub mod provider;
pub mod quality;
pub mod queue;
pub mod rating;
pub mod reconfigure;
pub mod recording;
pub mod recyclarr;
pub mod region;
pub mod repair;
pub use crate::error::retry;
pub mod rollback;
pub(crate) mod schemes;
pub mod screening;
pub mod secret;
pub mod seed;
pub mod self_update;
pub(crate) mod settings;
pub mod shell;
/// Moves every value the stack's environment file holds for a part or a published setting
/// into the stores, once.
pub use settings::filing::stack as filed_stack;
/// The environment file with every name a setting the core holds was read into
/// answering what the settings store holds.
pub use settings::legacy::overlaid as stored_over;
/// Every setting and secret the services read, as `lemonfiber settings` lists them.
pub use settings::run::listing as listed_settings;
/// Every refusal a change to a setting answers with before it is weighed.
pub use settings::REFUSALS as SETTINGS_REFUSALS;
pub mod space;
pub mod spoken;
pub mod stack;
pub mod storage;
pub mod stored;
pub mod telling;
#[cfg(test)]
pub mod test_support;
pub mod wishing;

// So the context builder the core's tests share with every other crate's can name this
// crate the way the others do.
#[cfg(test)]
extern crate self as lemonfiber_core;
pub mod text;
pub mod topology;
pub mod transcoding;
pub mod uninstall;
pub mod unmanaged;
pub mod update;
pub mod validate;
mod version;
pub mod walkthrough;
pub mod wiring;
pub mod within;
pub mod wizard;

/// The name of the product, as it appears to users.
pub const PRODUCT: &str = "lemonfiber";

/// Where this project's own image is published, each release under its version.
pub const IMAGE: &str = "ghcr.io/lemonfiber/lemonfiber";
