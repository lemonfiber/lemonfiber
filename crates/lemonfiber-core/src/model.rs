//! The values a surface renders.
//!
//! One set of types, serialised directly. `--json` and the web API are the same
//! values rather than two hand-maintained projections of them, which is what
//! makes the web API and the TUI's interface the same thing by construction —
//! and gives the machine-readable contract exactly one thing to version.

/// The machine-readable output contract's version.
///
/// Additive change leaves it alone, so a script asserting `== 1` keeps working
/// as features are added. Removing or retyping a field increments it.
pub const API_VERSION: u32 = 1;

mod admission;
mod alerts;
mod asking;
mod checking;
mod door;
mod envelope;
mod history;
mod hosting;
mod household;
mod invitation;
mod job;
pub mod kind;
mod migration;
mod provenance;
mod quality;
mod queue;
mod running;
mod self_update;
mod settings;
mod trace;
mod upgrade;
mod walkthrough;

pub use admission::*;
pub use alerts::*;
pub use asking::*;
pub use checking::*;
pub use door::*;
pub use envelope::*;
pub use history::*;
pub use hosting::*;
pub use household::*;
pub use invitation::*;
pub use job::*;
pub use migration::*;
pub use provenance::*;
pub use quality::*;
pub use queue::*;
pub use running::*;
pub use self_update::*;
pub use settings::*;
pub use trace::*;
pub use upgrade::*;
pub use walkthrough::*;

#[cfg(test)]
mod tests {
    use super::{kind, Envelope, VersionReport, API_VERSION};

    /// These are plain data, so serialising cannot fail; an empty string on the
    /// impossible branch keeps the helper free of a line no test can cover.
    fn json<T: serde::Serialize>(envelope: &Envelope<T>) -> String {
        envelope.to_json().unwrap_or_default()
    }

    #[test]
    fn every_payload_carries_the_contract_version() {
        let envelope = Envelope::new(kind::VERSION, 7_u32);
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
            changelog: crate::changelog::Notes::unread(),
        };
        assert_eq!(
            json(&Envelope::new(kind::VERSION, report)),
            concat!(
                r#"{"api_version":1,"kind":"version","data":{"binary":"0.1.0","#,
                r#""supported_schema":[1],"stack":"0.1.0","#,
                r#""compose":"Docker Compose version v2.32.1","#,
                r#""changelog":{"state":"stale","running":null,"releases":[],"#,
                r#""requirements":{}}}}"#
            )
        );
    }

    /// A run against this machine names no host, and one against another names it.
    ///
    /// Asserted on the wrapper directly rather than through what the run settled,
    /// because that is a value latched for the whole process: a test that set it
    /// would be deciding for every test beside it, and the wrapper is the thing
    /// under test either way.
    #[test]
    fn a_payload_about_another_machine_says_which_one() {
        assert_eq!(
            Envelope::new(kind::VERSION, 7_u32).host,
            None,
            "a run against this machine carries no host to mistake"
        );
        assert_eq!(
            super::settle_host(None),
            None,
            "and settling nothing leaves it that way"
        );

        let named = Envelope {
            host: Some("ssh://media@nas.local".to_owned()),
            ..Envelope::new(kind::VERSION, 7_u32)
        };
        assert_eq!(
            json(&named),
            r#"{"api_version":1,"kind":"version","data":7,"host":"ssh://media@nas.local"}"#
        );
    }

    #[test]
    fn an_unreachable_engine_is_absent_rather_than_guessed_at() {
        let report = VersionReport {
            binary: "0.1.0".to_owned(),
            supported_schema: vec![1],
            stack: "0.1.0".to_owned(),
            compose: None,
            changelog: crate::changelog::Notes::unread(),
        };
        assert!(json(&Envelope::new(kind::VERSION, report)).contains(r#""compose":null"#));
    }
}
