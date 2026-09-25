//! The binary driven from outside: what it carries, what it prints, and where a request goes.
//!
//! One binary rather than one per file: each file is a module here, so the
//! suite links once and the shared fakes are compiled once.

mod a_rehearsal_changes_nothing;
mod every_plugin_step_runs_unattended;
mod piped;
mod prompting;
mod publishing;
mod what_a_manifest_claims;
mod what_this_binary_carries;
mod where_a_request_goes;
