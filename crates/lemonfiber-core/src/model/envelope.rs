//! The wrapper every machine-readable payload arrives in.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

use super::kind::Kind;
use super::API_VERSION;

/// The wrapper every machine-readable payload arrives in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Envelope<T> {
    /// The output contract's version.
    pub api_version: u32,
    /// Which payload this is, so a consumer can branch before parsing `data`.
    // Described as the string it writes rather than as the type that holds it. The
    // type is a rule about which kinds may exist in this source; a caller reads a
    // string either way. This comment is not a doc comment because schemars
    // publishes those into the artefact, and the artefact describes the reply.
    #[schemars(with = "String")]
    pub kind: Kind,
    /// The payload.
    pub data: T,
}

impl<T: Serialize> Envelope<T> {
    /// Render this payload as the machine-readable contract.
    ///
    /// Rendering lives here rather than in a surface so there is one
    /// implementation of the contract rather than one per surface, and so a
    /// surface needs no JSON library to satisfy it.
    ///
    /// `None` only if a payload cannot serialise, which for these types cannot
    /// happen — they are plain data with no maps keyed by anything unusual.
    #[must_use]
    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string(self).ok()
    }
}

impl<T> Envelope<T> {
    /// Wrap a payload for machine-readable output.
    #[must_use]
    pub const fn new(kind: Kind, data: T) -> Self {
        Self {
            api_version: API_VERSION,
            kind,
            data,
        }
    }
}
