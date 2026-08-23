//! What starting long-running work answers with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.
//!
//! An action that reaches the container engine or a service runs for minutes, and a
//! reply that waited for it would tie the work to whoever asked. So the reply is a
//! name for the work instead, and what happens to that request afterwards — answered,
//! abandoned, a browser tab closed — cannot reach what it started.

use serde::Serialize;

/// Work that outlives the request that started it: its name, and what it was.
///
/// The action is carried beside the name because a client holding several has to
/// tell them apart, and asking it to remember which name it gave which request is
/// asking it to keep a second copy of what this already knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Started {
    /// The name to follow this work by on the event stream.
    pub job: String,
    /// The action that was asked for, as it was named.
    pub action: String,
}
