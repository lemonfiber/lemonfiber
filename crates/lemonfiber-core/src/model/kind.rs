//! What an envelope calls itself.
//!
//! A `kind` is named here and nowhere else. Naming it at the emit site and again
//! in the contract is two literals that can drift, and the drift is a contract
//! describing a kind nobody emits.

/// One moment of what the stack is doing, as the dashboard assembles it.
pub const DASHBOARD: &str = "dashboard";
/// A command could not do what was asked.
pub const ERROR: &str = "error";
/// One line of a service's log.
pub const LOG: &str = "log";
/// What setup settled on.
pub const SETUP: &str = "setup";
/// A walkthrough's outcome.
pub const WALKTHROUGH: &str = "walkthrough";
/// A supervision run's findings.
pub const WATCH: &str = "watch";
/// One glossary term.
pub const WORD: &str = "word";

/// Every kind, so the contract cannot describe one that is never emitted.
pub const ALL: &[&str] = &[DASHBOARD, ERROR, LOG, SETUP, WALKTHROUGH, WATCH, WORD];
