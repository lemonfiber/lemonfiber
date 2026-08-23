//! The machine-readable description of what the surfaces exchange.
//!
//! Every SDK generates its types from this rather than transcribing them, so a
//! field added here reaches every client without anyone retyping it. The types
//! described are the ones that actually serialise the reply, which is what stops
//! the description drifting from the thing it describes.
//!
//! The shapes are generated rather than written, and regenerating must
//! produce no diff — a serialised type that changes without the artefact
//! changing with it fails the build instead of reaching an SDK.

use std::collections::BTreeMap;

use schemars::{schema_for, Schema};
use serde::Serialize;

use crate::glossary::Term;
use crate::model::{
    kind, Envelope, SetupReport, SupervisionReport, WalkthroughReport, API_VERSION,
};
use crate::ports::docker::LogLine;
use crate::ports::error::Problem;

/// Where the generated artefact is kept, relative to the workspace root.
pub const CONTRACT_PATH: &str = "contract/web-api.contract.json";

/// Every wire shape a surface may receive, keyed by its `kind`.
///
/// Each entry is the whole envelope with that kind's payload in place, rather
/// than the payload alone: a generator wants the shape it will actually parse.
#[derive(Debug, Serialize)]
pub struct Contract {
    /// The wire version these shapes belong to.
    pub api_version: u32,
    /// `kind` to the schema of the envelope carrying it.
    pub kinds: BTreeMap<String, Schema>,
}

impl Contract {
    /// Builds the contract from the types that serialise the reply.
    #[must_use]
    pub fn describe() -> Self {
        let mut kinds = BTreeMap::new();
        kinds.insert(kind::ERROR.to_owned(), schema_for!(Envelope<Problem>));
        kinds.insert(kind::LOG.to_owned(), schema_for!(Envelope<LogLine>));
        kinds.insert(kind::SETUP.to_owned(), schema_for!(Envelope<SetupReport>));
        kinds.insert(
            kind::WALKTHROUGH.to_owned(),
            schema_for!(Envelope<WalkthroughReport>),
        );
        kinds.insert(
            kind::WATCH.to_owned(),
            schema_for!(Envelope<SupervisionReport>),
        );
        kinds.insert(kind::WORD.to_owned(), schema_for!(Envelope<Term>));

        Self {
            api_version: API_VERSION,
            kinds,
        }
    }

    /// As it is committed: sorted keys, two-space indent, one trailing newline.
    ///
    /// `None` only if it cannot serialise, which a tree of schemas cannot.
    #[must_use]
    pub fn to_json(&self) -> Option<String> {
        let mut text = serde_json::to_string_pretty(self).ok()?;
        text.push('\n');
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::{Contract, CONTRACT_PATH};

    /// What is committed, read from the workspace root.
    fn committed() -> Option<String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::read_to_string(root.join(CONTRACT_PATH)).ok()
    }

    /// The committed artefact and the types must agree.
    ///
    /// A change to a serialised shape that forgets to regenerate fails here
    /// rather than reaching an SDK.
    #[test]
    fn the_committed_contract_still_matches_the_types() {
        let fresh = Contract::describe().to_json().unwrap_or_default();
        let stored = committed().unwrap_or_default();

        assert_eq!(
            stored, fresh,
            "the contract is out of date — regenerate it with `just contract`"
        );
    }

    /// The contract and the emitters must name the same set of kinds.
    ///
    /// Describing a kind nobody emits, or emitting one the contract omits, are
    /// both silent: each half is self-consistent, so only comparing them shows it.
    #[test]
    fn it_describes_every_kind_that_is_emitted_and_no_others() {
        let contract = Contract::describe();
        let described: Vec<&str> = contract.kinds.keys().map(String::as_str).collect();
        let mut emitted: Vec<&str> = crate::model::kind::ALL.to_vec();
        emitted.sort_unstable();

        assert_eq!(described, emitted);
    }

    #[test]
    fn it_describes_the_wire_version_it_belongs_to() {
        assert_eq!(Contract::describe().api_version, crate::model::API_VERSION);
    }

    #[test]
    fn every_kind_carries_the_whole_envelope_not_just_its_payload() {
        let contract = Contract::describe();
        let text = contract.to_json().unwrap_or_default();

        assert!(contract.kinds.contains_key("word"), "{:?}", contract.kinds);
        assert!(text.contains("api_version"), "{text}");
        assert!(text.contains("kind"), "{text}");
    }

    #[test]
    fn it_is_written_the_same_way_twice() {
        let once = Contract::describe().to_json();
        let twice = Contract::describe().to_json();

        assert_eq!(once, twice);
        assert!(once.unwrap_or_default().ends_with("}\n"));
    }
}
