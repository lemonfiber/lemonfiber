//! What the command line asks the core for itself, as each answer is written.

use lemonfiber_core::app::password::PasswordReport;
use lemonfiber_core::app::{InFlightReport, LogsReport, Outcome, Unfinished};
use lemonfiber_core::companion::served::ServedReport;
use lemonfiber_core::dashboard::Protocol;
use lemonfiber_core::walkthrough::{Shape, Why};

/// What each command the command line dispatches for itself answers with.
pub(crate) fn samples() -> Vec<Outcome> {
    vec![
        Outcome::Password(PasswordReport {
            device_keys_revoked: true,
        }),
        Outcome::WalkthroughOffer(Why::Offer(Shape::Pipeline)),
        Outcome::Logs(LogsReport { lines: 2 }),
        Outcome::InFlight(InFlightReport {
            downloads: vec![Unfinished {
                protocol: Protocol::Usenet,
                name: "Big.Buck.Bunny.2008.1080p".to_owned(),
                progress: 42,
            }],
        }),
        Outcome::Served(ServedReport {
            fingerprint: Some("6642e3f45a1d89c0".to_owned()),
            named: Some("lemonfiber.local".to_owned()),
            unrecorded: None,
            presented: None,
        }),
    ]
}
