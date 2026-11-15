//! What the contract carries beside its schemas: one example of each kind and each
//! action, as a run against the core's fixture fakes answered it.
//!
//! The examples are not built here. A harness outside the shipped crates runs every
//! command against the fakes and hands this what each answered; this only says what
//! an example is and where in the directory it is kept, so the schemas and the
//! examples of one kind are written by the same index.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};

/// Where the examples are, inside the contract's directory.
pub const EXAMPLES: &str = "examples";

/// Every example a harness wrote, keyed by the kind or the action it is one of.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Examples {
    /// Each kind, to an envelope of that kind a run answered with.
    pub kinds: BTreeMap<String, Value>,
    /// Each action, to what it was sent with and what it answered.
    pub actions: BTreeMap<String, Acted>,
}

/// One action as a run took it: the arguments it was sent with and the envelope it
/// answered.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Acted {
    /// The request body, each argument by the name the action lists it under.
    pub arguments: Map<String, Value>,
    /// The envelope the action answered with.
    pub answered: Value,
}

/// Where each example is kept: each kind and each action to its file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Kept {
    /// Each kind, to the file holding its example.
    pub kinds: BTreeMap<String, String>,
    /// Each action, to the file holding its example.
    pub actions: BTreeMap<String, String>,
}

/// The file the example of `kind` is kept in.
#[must_use]
pub fn of_kind(kind: &str) -> String {
    format!("{EXAMPLES}/kinds/{kind}.json")
}

/// The file the example of `action` is kept in.
#[must_use]
pub fn of_action(action: &str) -> String {
    format!("{EXAMPLES}/actions/{action}.json")
}
