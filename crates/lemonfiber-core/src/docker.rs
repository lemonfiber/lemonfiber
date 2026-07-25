//! Making sense of what the engine reports.
//!
//! The port returns containers, samples and log lines; this module turns them
//! into the state a surface renders — correlating containers back to the
//! services that declared them, and deciding what "started" actually means.
//!
//! Startup is gated on health rather than on the process existing, so a service
//! that is up but not yet answering is not reported as running.
//!
//! Arrives with the dashboard. See `.docs/architecture/module-layout.md`.
