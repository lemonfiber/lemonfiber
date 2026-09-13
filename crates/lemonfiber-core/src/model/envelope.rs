//! The wrapper every machine-readable payload arrives in.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use std::sync::OnceLock;

use serde::Serialize;

use super::kind::Kind;
use super::API_VERSION;

/// Which machine this run is operating, where it is not this one.
///
/// A latch rather than an argument, for the reason the surfaces settle their own
/// audience that way: it is a property of the run and not of any payload. Every
/// command's answer has to name the host, and threading it through the twelve
/// places that wrap one is twelve chances to be told wrong — with the one that was
/// missed being the command that quietly reported on the wrong machine, which is
/// the entire failure this is here to prevent.
static OPERATING: OnceLock<Option<String>> = OnceLock::new();

/// Settle which host this run's payloads name, and say what is in force.
///
/// The first call decides. A later one is ignored and told what the first settled,
/// because a surface cannot change which machine a run is operating halfway through
/// answering about it, and should not be told that it did.
///
/// `None` for a run against this machine, which is the ordinary case: naming this
/// machine in every payload would be noise, and a consumer that sees a host knows
/// from its presence alone that the answer is about somewhere else.
#[must_use]
pub fn settle_host(host: Option<String>) -> Option<String> {
    OPERATING.get_or_init(|| host).clone()
}

/// The host this run operates, as a payload should carry it.
fn operating() -> Option<String> {
    OPERATING.get().cloned().flatten()
}

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
    /// The machine this answer is about, where it is not the one lemonfiber runs on.
    // Present exactly when a remote Docker context is in force, so a consumer can
    // tell a report about the server from one about the laptop without being told
    // separately; absent otherwise, which leaves every existing document the shape
    // it already had. Whatever a credential could ride in is taken out before it
    // reaches here, because an endpoint is a URL and a URL carries a password in
    // front of its host and a token in its query. Said here rather than in the doc
    // comment for the reason the field above says: schemars publishes those into
    // the artefact, and the artefact describes the reply rather than this decision.
    //
    // Skipped when absent rather than written as null, because "leaves every existing
    // document the shape it already had" is a claim a null would falsify: every reply
    // from a local run would gain a field, and the one test that reads the envelope
    // whole would be the only thing that noticed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
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
    ///
    /// The host is read from what the run settled rather than taken as an argument,
    /// so no caller can wrap a payload and forget to say which machine it is about.
    #[must_use]
    pub fn new(kind: Kind, data: T) -> Self {
        Self {
            api_version: API_VERSION,
            kind,
            data,
            host: operating(),
        }
    }
}
