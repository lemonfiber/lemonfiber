//! The adapters driven against stand-ins for what they reach: an engine socket, a process.
//!
//! One binary rather than one per file: each file is a module here, so the
//! suite links once and the shared fakes are compiled once.

mod engine;
mod exec;
mod fake;
