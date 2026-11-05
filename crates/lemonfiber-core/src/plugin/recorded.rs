//! A response somebody recorded, as the core reads one against a manifest's assertions.
//!
//! The recording itself, its reader and how it is named are
//! [`lemonfiber_conformance::recording`]'s: one format, read alike by the claims a plugin
//! makes, the contracts its adapters speak and the build that holds the bundle to both.

pub use lemonfiber_conformance::recording::{
    of, read, runs, said, Answer, Asked, Of, Recording, RunAsked, RunResult,
};

/// Whether a recording is of the request an assertion asks.
///
/// A recording of some other call says nothing about this one. The two have drifted,
/// which is a thing to report rather than to judge either way.
///
/// What was asked for is part of *which call this is*, not a detail beside it. A
/// recording taken without `Accept: application/json` from a service that answers XML
/// without it is a recording of the XML, and running a JSON assertion against it would
/// fail a probe over a header nobody sent.
#[must_use]
pub fn records(recorded: &Asked, asked: &lemonfiber_plugin::Request) -> bool {
    recorded.method == asked.method
        && recorded.path == asked.path
        && recorded.accept == asked.accept
}
