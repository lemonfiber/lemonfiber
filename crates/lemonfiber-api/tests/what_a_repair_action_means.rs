//! What a browser may ask to be put right, and what it has to have read first.
//!
//! Three actions, and the one thing about them that is this surface's own problem
//! rather than the core's: **a terminal holds the question open and this does not.**
//!
//! At a terminal the offer is printed, the operator answers, and the run that acts
//! is the run that looked. Here the offer is read in one request and the answer
//! arrives in another, so the answer has to say which offer it was answering — and
//! the run that acts looks again and refuses to spend consent on an offer that has
//! moved on since. Nothing is held between the two requests, which is why a browser
//! tab closed halfway through leaves nothing half-consented.
//!
//! The three shapes are the three the command line spells. Without the yes it is
//! the offer, which changes nothing. With the yes and the offer it was read in, it
//! is consent to that offer. With the yes alone it is `--yes`: a decision taken
//! before there was anything to read, which is not the same as skipping being told.
//!
//! Driven from outside the crate, because what a caller can reach is the thing
//! worth holding still.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::{header, StatusCode};
use lemonfiber_api::actions;
use lemonfiber_api::actions::{answering, named, Answering, Arguments, Disturbing, Refused};
use lemonfiber_api::events::live::Live;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::repair::Consent;
use lemonfiber_core::app::{Command, Ctx};
use lemonfiber_core::config::Settings;
use lemonfiber_core::doctor::Narrowing;
use lemonfiber_core::platform::Environment;
use lemonfiber_fixtures::ports::{Chance, Idle, Stopped};

/// An offer, as one names itself.
const OFFER: &str = "0f0f0f0f";

/// A check by the name its finding gives it.
const CHECK: &str = "vpn.port-forward-client";

/// What an action came to, or nothing where it was refused.
fn command(action: &str, given: Arguments) -> Option<Command> {
    named(action, given).ok()
}

/// Why an action was refused, or nothing where it was not.
fn refusal(action: &str, given: Arguments) -> Option<Refused> {
    named(action, given).err()
}

/// A repairing run, as the fields a request carries.
fn repairing(confirm: bool, offer: Option<&str>, agreed: &[&str], disruptive: bool) -> Arguments {
    Arguments {
        confirm,
        offer: offer.map(str::to_owned),
        agreed: agreed.iter().map(|check| (*check).to_owned()).collect(),
        disruptive: disruptive.into(),
        ..Arguments::default()
    }
}

// ── The three consents, and nothing in between ────────────────────────────────

#[test]
fn a_run_that_names_no_yes_is_the_offer_and_changes_nothing() {
    assert_eq!(
        command("repair", repairing(false, None, &[], false)),
        Some(Command::Repair {
            consent: Consent::Offer,
            disruptive: false,
        })
    );
}

#[test]
fn a_yes_with_no_offer_named_is_the_standing_consent_decided_in_advance() {
    // `--yes` on the command line: carried out without being asked about each,
    // because the decision was taken before there was an offer to read.
    assert_eq!(
        command("repair", repairing(true, None, &[], false)),
        Some(Command::Repair {
            consent: Consent::Standing,
            disruptive: false,
        })
    );
}

#[test]
fn a_yes_that_names_the_offer_it_answered_is_consent_to_that_offer() {
    assert_eq!(
        command("repair", repairing(true, Some(OFFER), &[CHECK], false)),
        Some(Command::Repair {
            consent: Consent::Given {
                offer: OFFER.to_owned(),
                repairs: vec![CHECK.to_owned()],
            },
            disruptive: false,
        })
    );
}

#[test]
fn consent_that_names_no_offer_is_refused_rather_than_read_charitably() {
    // An answer that does not say which offer it answered cannot be held against
    // the offer that stands, so it is the one thing this surface cannot let pass.
    assert_eq!(
        refusal("repair", repairing(true, None, &[CHECK], false)),
        Some(Refused::Missing {
            action: "repair".to_owned(),
            argument: "offer".to_owned(),
        })
    );
}

#[test]
fn an_offer_nobody_agreed_to_any_of_is_refused_as_having_lost_its_subject() {
    assert_eq!(
        refusal("repair", repairing(true, Some(OFFER), &[], false)),
        Some(Refused::Missing {
            action: "repair".to_owned(),
            argument: "agreed".to_owned(),
        })
    );
}

#[test]
fn repairs_named_without_a_yes_are_refused_rather_than_carried_out() {
    // Naming what you would agree to is not agreeing to it, and a surface that read
    // it as agreement would be changing a machine on the strength of a list.
    for asked in [
        repairing(false, Some(OFFER), &[CHECK], false),
        repairing(false, None, &[CHECK], false),
        repairing(false, Some(OFFER), &[], false),
    ] {
        assert_eq!(
            refusal("repair", asked),
            Some(Refused::Missing {
                action: "repair".to_owned(),
                argument: "confirm".to_owned(),
            })
        );
    }
}

// ── Widening the suite is a separate decision from agreeing to a repair ───────

#[test]
fn including_the_checks_that_disturb_is_carried_apart_from_the_consent() {
    // `--fix-disruptive` widens what is looked at; it agrees to nothing. Carrying it
    // changes no consent into another, which is what this holds still.
    //
    // Whether a given consent may spend it is the core's, not this translation's:
    // an offer asked to disturb is refused there, once, so the command line's own
    // machine-readable offer is refused by the same rule rather than by a second
    // copy of it here.
    for (asked, consent) in [
        (repairing(false, None, &[], true), Consent::Offer),
        (repairing(true, None, &[], true), Consent::Standing),
        (
            repairing(true, Some(OFFER), &[CHECK], true),
            Consent::Given {
                offer: OFFER.to_owned(),
                repairs: vec![CHECK.to_owned()],
            },
        ),
    ] {
        assert_eq!(
            command("repair", asked),
            Some(Command::Repair {
                consent,
                disruptive: true,
            })
        );
    }
}

// ── Undo is an errand of its own, and carries no subject ──────────────────────

#[test]
fn putting_the_last_repair_back_takes_nothing_at_all() {
    assert_eq!(command("undo", Arguments::default()), Some(Command::Undo));
}

#[test]
fn an_undo_told_which_repair_to_reverse_is_refused_rather_than_obliged() {
    // Which repair was last is the core's to decide, so there is nothing here for a
    // caller to name — and a name it accepted and dropped would answer a different
    // request from the one asked for.
    for (argument, given) in [
        (
            "agreed",
            Arguments {
                agreed: vec![CHECK.to_owned()],
                ..Arguments::default()
            },
        ),
        (
            "confirm",
            Arguments {
                confirm: true,
                ..Arguments::default()
            },
        ),
        (
            "disruptive",
            Arguments {
                disruptive: Disturbing::Included,
                ..Arguments::default()
            },
        ),
        (
            "forms",
            Arguments {
                forms: vec!["tv".to_owned()],
                ..Arguments::default()
            },
        ),
    ] {
        assert_eq!(
            refusal("undo", given),
            Some(Refused::Unwanted {
                action: "undo".to_owned(),
                argument: argument.to_owned(),
            })
        );
    }
}

// ── Answering a warning is a write, and says which warning ────────────────────

#[test]
fn accepting_a_warning_names_the_check_it_is_answering() {
    let asked = Arguments {
        check: Some("vpn.unprotected".to_owned()),
        ..Arguments::default()
    };
    assert_eq!(
        command("accept", asked),
        Some(Command::Doctor {
            narrowing: Narrowing::Suite,
            disruptive: false,
            accept: Some("vpn.unprotected".to_owned()),
        })
    );
}

#[test]
fn accepting_a_warning_a_disturbing_check_raised_says_to_run_those_too() {
    // Only something this run warns about can be answered, so a warning that only
    // the disturbing checks raise needs them run — which is why this action carries
    // the widening and the read that serves the diagnosis does not.
    let asked = Arguments {
        check: Some("vpn.killswitch".to_owned()),
        disruptive: Disturbing::Included,
        ..Arguments::default()
    };
    assert_eq!(
        command("accept", asked),
        Some(Command::Doctor {
            narrowing: Narrowing::Suite,
            disruptive: true,
            accept: Some("vpn.killswitch".to_owned()),
        })
    );
}

#[test]
fn accepting_nothing_in_particular_is_refused_rather_than_read_as_a_diagnosis() {
    assert_eq!(
        refusal("accept", Arguments::default()),
        Some(Refused::Missing {
            action: "accept".to_owned(),
            argument: "check".to_owned(),
        })
    );
}

// ── All three run for minutes, so none of them is waited for ──────────────────

#[test]
fn every_repairing_run_is_a_wait_an_operator_can_watch() {
    // Each of them reaches the services: the offer runs the checks, the consent
    // runs them and acts, and a reversal puts a field back through the service that
    // holds it. A request that waited for any of them would tie the work to one
    // connection.
    for asked in [
        command("repair", repairing(false, None, &[], false)),
        command("repair", repairing(true, None, &[], false)),
        command("repair", repairing(true, Some(OFFER), &[CHECK], false)),
        command("undo", Arguments::default()),
        command(
            "accept",
            Arguments {
                check: Some("vpn.unprotected".to_owned()),
                ..Arguments::default()
            },
        ),
    ] {
        let Some(command) = asked else {
            unreachable!("each of these names an action this surface offers");
        };
        assert_eq!(answering(&command), Answering::Later);
    }
}

// ── The routes themselves, driven without a socket ────────────────────────────

/// A context over a stack that will not read, which is enough to prove which of the
/// two answers a request gets and under which name.
fn ctx() -> Ctx {
    Ctx::new(
        Arc::new(Idle),
        Arc::new(lemonfiber_core::adapters::Daemon::local()),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        lemonfiber_core::stack::Source::External(std::path::Path::new("/lemonfiber/no/such/stack")),
        Settings::default(),
        Environment::MacOs,
    )
    .with_random(Arc::new(Chance::cycling()))
}

/// The action routes as a run builds them, over a context a test chose.
fn routed() -> axum::Router {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    actions::routes().with_state(Serving {
        ctx: Arc::new(ctx()),
        token: Arc::new(token),
        bound: ([127, 0, 0, 1], 8472).into(),
        admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
        jobs: Jobs::default(),
        live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
    })
}

/// What the route answered, as the status it answered under and what it said.
async fn said(action: &str, body: &str) -> (u16, String) {
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/actions/{action}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body.to_owned()));
    let Ok(request) = request else {
        unreachable!("a request built from values that are already headers cannot fail");
    };
    let served = tower::ServiceExt::oneshot(routed(), request).await.ok();
    let Some(response) = served else {
        unreachable!("the router is infallible; its handlers answer rather than fail");
    };
    let status = response.status().as_u16();
    let read = to_bytes(response.into_body(), usize::MAX).await;
    let bytes = read.map(|bytes| bytes.to_vec()).unwrap_or_default();
    (status, String::from_utf8(bytes).unwrap_or_default())
}

#[tokio::test]
async fn each_repairing_run_is_answered_with_a_name_for_the_work() {
    let asked = [
        ("repair", "{}".to_owned()),
        ("repair", r#"{"confirm":true}"#.to_owned()),
        (
            "repair",
            format!(r#"{{"confirm":true,"offer":"{OFFER}","agreed":["{CHECK}"]}}"#),
        ),
        ("undo", "{}".to_owned()),
        ("accept", r#"{"check":"vpn.unprotected"}"#.to_owned()),
    ];
    for (action, body) in asked {
        let (status, said) = said(action, &body).await;
        assert_eq!(status, StatusCode::ACCEPTED.as_u16(), "{action}: {said}");
        assert!(said.contains(r#""kind":"job""#), "{action}: {said}");
        assert!(
            said.contains(&format!(r#""action":"{action}""#)),
            "{action}: {said}"
        );
    }
}

#[tokio::test]
async fn an_answer_that_does_not_say_which_offer_it_answered_is_refused_by_name() {
    let (status, said) = said("repair", r#"{"confirm":true,"agreed":["vpn.killswitch"]}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST.as_u16());
    assert!(said.contains("`offer`"), "{said}");
}

#[tokio::test]
async fn an_argument_a_reversal_does_not_take_is_refused_rather_than_dropped() {
    let (status, said) = said("undo", r#"{"confirm":true}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST.as_u16());
    assert!(said.contains("takes no `confirm`"), "{said}");
}

#[tokio::test]
async fn a_name_the_carrier_does_not_hold_is_refused_before_anything_else() {
    // `--fix` is what the command line calls the flag; the consent is the argument
    // here, and a caller that sent the flag's name is told rather than having a
    // whole request quietly mean something else.
    let (status, _) = said("repair", r#"{"fix":true}"#).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY.as_u16());
}
