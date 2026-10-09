//! The cases an adapter's recordings and its live pass must answer for one capability.
//!
//! Every operation answers inside its contract, and every adapter refuses a call made
//! without its key.

use crate::{Capability, Operation};

/// The statuses an adapter refuses a call without its key with.
const UNKEYED: &[u16] = &[401];

/// What a case holds an answer to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// Its status is one of these.
    Status(&'static [u16]),
    /// It is an answer the operation's contract declares.
    Conforms,
}

/// One case of a capability's conformance.
#[derive(Debug, Clone)]
pub struct Case<'c> {
    /// Its name, which its recording is filed under.
    pub case: String,
    /// The operation it asks.
    pub operation: &'c Operation,
    /// Whether it is asked under the plugin's key.
    pub keyed: bool,
    /// What its answer is held to.
    pub expect: Expect,
    /// Whether install asks it of the running adapter as well.
    pub live: bool,
}

/// Every case of `capability`, in the order they are published.
#[must_use]
pub fn cases(capability: &Capability) -> Vec<Case<'_>> {
    let unkeyed = capability.operations.first().map(|operation| Case {
        case: "refuses-without-the-key".to_owned(),
        operation,
        keyed: false,
        expect: Expect::Status(UNKEYED),
        live: true,
    });
    unkeyed
        .into_iter()
        .chain(capability.operations.iter().map(|operation| Case {
            case: format!("{}-answers", operation.name),
            operation,
            keyed: true,
            expect: Expect::Conforms,
            live: false,
        }))
        .collect()
}

#[cfg(test)]
mod tests;
