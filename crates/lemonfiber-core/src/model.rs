//! The values a surface renders.
//!
//! One set of types, serialised directly. `--json` and the web API are the same
//! values rather than two hand-maintained projections of them, which is what
//! makes the web API and the TUI's interface the same thing by construction —
//! and gives the machine-readable contract exactly one thing to version.

use serde::Serialize;

/// The machine-readable output contract's version.
///
/// Additive change leaves it alone, so a script asserting `== 1` keeps working
/// as features are added. Removing or retyping a field increments it.
pub const API_VERSION: u32 = 1;

/// The wrapper every machine-readable payload arrives in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Envelope<T> {
    /// The output contract's version.
    pub api_version: u32,
    /// Which payload this is, so a consumer can branch before parsing `data`.
    pub kind: &'static str,
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
    pub const fn new(kind: &'static str, data: T) -> Self {
        Self {
            api_version: API_VERSION,
            kind,
            data,
        }
    }
}

/// What versions are in play: the binary, and the stack it can operate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersionReport {
    /// The running binary's version.
    pub binary: String,
    /// The manifest schema generations this build reads.
    pub supported_schema: Vec<u32>,
    /// The version of the stack this build operates.
    pub stack: String,
    /// What the container engine reports, when it could be asked.
    pub compose: Option<String>,
}

/// What a lifecycle command did, or would have done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LifecycleReport {
    /// The Compose subcommand that was run.
    pub action: String,
    /// The profiles that were activated.
    pub profiles: Vec<String>,
    /// Profiles the forms asked for that the configuration does not support.
    ///
    /// Reported rather than dropped quietly: an operator seeing fewer services
    /// than they expected needs to be told which, and why, before they go
    /// looking for a fault that is not there.
    pub dropped: Vec<String>,
    /// The exact command, so what happened is never a matter of trust.
    pub command: Vec<String>,
    /// Whether this was a rehearsal.
    pub rehearsed: bool,
    /// The exit status, absent for a rehearsal or a signalled process.
    pub status: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::{Envelope, VersionReport, API_VERSION};

    /// These are plain data, so serialising cannot fail; an empty string on the
    /// impossible branch keeps the helper free of a line no test can cover.
    fn json<T: serde::Serialize>(envelope: &Envelope<T>) -> String {
        envelope.to_json().unwrap_or_default()
    }

    #[test]
    fn every_payload_carries_the_contract_version() {
        let envelope = Envelope::new("version", 7_u32);
        assert_eq!(envelope.api_version, API_VERSION);
        assert_eq!(
            json(&envelope),
            r#"{"api_version":1,"kind":"version","data":7}"#
        );
    }

    #[test]
    fn a_version_report_serialises_field_for_field() {
        let report = VersionReport {
            binary: "0.1.0".to_owned(),
            supported_schema: vec![1],
            stack: "0.1.0".to_owned(),
            compose: Some("Docker Compose version v2.32.1".to_owned()),
        };
        assert_eq!(
            json(&Envelope::new("version", report)),
            concat!(
                r#"{"api_version":1,"kind":"version","data":{"binary":"0.1.0","#,
                r#""supported_schema":[1],"stack":"0.1.0","#,
                r#""compose":"Docker Compose version v2.32.1"}}"#
            )
        );
    }

    #[test]
    fn an_unreachable_engine_is_absent_rather_than_guessed_at() {
        let report = VersionReport {
            binary: "0.1.0".to_owned(),
            supported_schema: vec![1],
            stack: "0.1.0".to_owned(),
            compose: None,
        };
        assert!(json(&Envelope::new("version", report)).contains(r#""compose":null"#));
    }
}
