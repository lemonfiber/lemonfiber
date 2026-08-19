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

mod checking;
mod envelope;
mod household;
mod quality;
mod queue;
mod running;
mod settings;
mod trace;
mod upgrade;
mod walkthrough;

pub use checking::*;
pub use envelope::*;
pub use household::*;
pub use quality::*;
pub use queue::*;
pub use running::*;
pub use settings::*;
pub use trace::*;
pub use upgrade::*;
pub use walkthrough::*;

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
