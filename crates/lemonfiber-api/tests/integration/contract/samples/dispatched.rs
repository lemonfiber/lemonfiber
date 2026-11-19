//! What a surface asks the core for itself, as each answer is written.

use lemonfiber_core::app::admitting::{Admitted, Answer, Caller, Calling, SignInCode};
use lemonfiber_core::app::password::PasswordReport;
use lemonfiber_core::app::standing::StackStanding;
use lemonfiber_core::app::{InFlightReport, LogsReport, Outcome, Unfinished};
use lemonfiber_core::companion::served::ServedReport;
use lemonfiber_core::dashboard::Protocol;
use lemonfiber_core::device_keys::Challenged;
use lemonfiber_core::instant::Instant;
use lemonfiber_core::keys::Scope;
use lemonfiber_core::walkthrough::{Shape, Why};

/// What each command a surface dispatches for itself answers with.
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
        Outcome::Standing(StackStanding {
            stack: Some("f3a1c2d4".to_owned()),
            switched_off: vec![lemonfiber_core::config::REACH_REGISTRY_KEY.to_owned()],
            told: true,
            waiting: false,
            members_mint: false,
        }),
        Outcome::Admitting(Answer::Admission(Admitted {
            token: "5e55".repeat(16),
            until: Instant::from_seconds(1_791_223_200),
            member: Some("a7f3".to_owned()),
            name: Some("Ana".to_owned()),
        })),
        Outcome::Admitting(Answer::SignInCode(SignInCode {
            code: Some("K7Q2".to_owned()),
            approve_at: Some("https://example.org/link".to_owned()),
            lapses_at: Instant::from_seconds(1_791_180_900),
            waiting: "0a".repeat(16),
        })),
        Outcome::Admitting(Answer::Caller(Calling::Known {
            caller: Caller::Key {
                name: "home-assistant".to_owned(),
                scope: Scope::Read,
            },
        })),
        Outcome::Admitting(Answer::Caller(Calling::Held { retry_after: 8 })),
        Outcome::Admitting(Answer::Challenge(Challenged {
            challenge: "c".repeat(43),
            expires: Instant::from_seconds(1_791_180_060),
        })),
    ]
}
