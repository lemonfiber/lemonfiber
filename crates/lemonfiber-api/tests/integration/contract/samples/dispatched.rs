//! What a surface asks the core for itself, as each answer is written.

use lemonfiber_core::app::admitting::{Admitted, Answer, Caller, Calling, SignInCode};
use lemonfiber_core::app::password::PasswordReport;
use lemonfiber_core::app::standing::StackStanding;
use lemonfiber_core::app::surfacing::Answer as Surfaced;
use lemonfiber_core::app::{InFlightReport, LogsReport, Outcome, Unfinished};
use lemonfiber_core::certificate::described::Described;
use lemonfiber_core::companion::served::ServedReport;
use lemonfiber_core::dashboard::Protocol;
use lemonfiber_core::device_keys::Challenged;
use lemonfiber_core::instant::Instant;
use lemonfiber_core::keys::Scope;
use lemonfiber_core::notify::events::Heard;
use lemonfiber_core::screening::{Renewed, Untuned};
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
            port: 8443,
            encrypted: true,
            network: true,
            named: Some("lemonfiber.local".to_owned()),
            unrecorded: None,
            certificate: Some(Described {
                fingerprint: "6642e3f45a1d89c0".repeat(4),
                subject: "CN=lemonfiber.local".to_owned(),
                subject_cut: false,
                issuer: "CN=lemonfiber.local".to_owned(),
                issuer_cut: false,
                self_signed: true,
                names: vec!["lemonfiber.local".to_owned()],
                names_more: 0,
                valid_from: Instant::from_seconds(157_766_400),
                valid_until: Instant::from_seconds(67_090_118_400),
            }),
            presented: None,
        }),
        Outcome::Surfaced(Surfaced::Standing(StackStanding {
            stack: Some("f3a1c2d4".to_owned()),
            switched_off: vec![lemonfiber_core::config::REACH_REGISTRY_KEY.to_owned()],
            told: true,
            waiting: false,
            members_mint: false,
        })),
        Outcome::Surfaced(Surfaced::Heard(Heard {
            service: "sonarr".to_owned(),
        })),
        Outcome::Surfaced(Surfaced::Renewed(Renewed {
            member: "a7f3".to_owned(),
            lasts_until: "2026-12-25".to_owned(),
            rehearsed: false,
        })),
        Outcome::Surfaced(Surfaced::Untuned(Untuned { closed: 2 })),
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
