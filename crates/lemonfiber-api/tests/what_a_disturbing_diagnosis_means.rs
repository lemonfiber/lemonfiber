//! The one diagnosis a browser asks for by changing something.
//!
//! Every other diagnosis on this surface is a `GET`: `/api/checks` runs the suite,
//! `/api/storage` runs the group about the disk, and both look without touching.
//! Widened to the checks that disturb a running system, the same request stops being
//! a read — the killswitch test takes the tunnel away to prove traffic stops without
//! it, and the releases check spends a real search against the indexers' daily
//! allowance. So it is asked for at the door changes are asked for, which is the only
//! door this surface has that is not a read.
//!
//! What that leaves this file to hold still is three things.
//!
//! **It is the same request the command line makes.** `lemonfiber doctor
//! --disruptive` reaches `Command::Doctor` with the widening set, and so does this;
//! nothing new was invented in the core for a browser, and nothing here assembles a
//! command of its own.
//!
//! **It cannot impersonate the read.** Asked without the widening it would be the
//! plain diagnosis, which is already served — so the widening is required rather than
//! defaulted, and a request that leaves it out is refused by name.
//!
//! **It can be narrowed the way the command line narrows it.** Both disturbing checks
//! tell the operator to run *that one*: the releases finding names
//! `--only services.releases --disruptive` outright. Without a narrowing here,
//! following that from a browser would mean dropping the tunnel as well.
//!
//! Driven from outside the crate, because what a caller can reach is the thing worth
//! holding still.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::{header, StatusCode};
use lemonfiber_api::actions;
use lemonfiber_api::actions::{answering, named, Answering, Arguments, Refused};
use lemonfiber_api::events::live::Live;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::{Command, Ctx};
use lemonfiber_core::config::Settings;
use lemonfiber_core::doctor::{Category, Narrowing};
use lemonfiber_core::platform::Environment;
use lemonfiber_fixtures::ports::{Chance, Idle, Stopped};

/// The name the action is asked for under.
const DIAGNOSE: &str = "diagnose";

/// One check that only a disturbing run reports a real verdict for.
const KILLSWITCH: &str = "vpn.killswitch";

/// A disturbing run, as the fields a request carries.
fn disturbing(only: Option<&str>) -> Arguments {
    Arguments {
        only: only.map(str::to_owned),
        disruptive: true.into(),
        ..Arguments::default()
    }
}

/// What an action came to, or nothing where it was refused.
fn command(action: &str, given: Arguments) -> Option<Command> {
    named(action, given).ok()
}

/// Why an action was refused, or nothing where it was not.
fn refusal(action: &str, given: Arguments) -> Option<Refused> {
    named(action, given).err()
}

// ── It is the command line's own request ──────────────────────────────────────

#[test]
fn a_disturbing_diagnosis_reaches_the_command_the_command_line_reaches() {
    // `lemonfiber doctor --disruptive`, with nothing narrowed and no warning
    // answered: the widening is the whole of what this asks for.
    assert_eq!(
        command(DIAGNOSE, disturbing(None)),
        Some(Command::Doctor {
            narrowing: Narrowing::Suite,
            disruptive: true,
            accept: None,
        })
    );
}

#[test]
fn one_group_of_checks_is_disturbed_rather_than_all_of_them() {
    // `lemonfiber doctor --only services --disruptive`. The live release search is
    // in this group and the killswitch test is not, so an operator following the
    // releases finding's own instruction keeps their tunnel.
    assert_eq!(
        command(DIAGNOSE, disturbing(Some("services"))),
        Some(Command::Doctor {
            narrowing: Narrowing::Category(Category::Services),
            disruptive: true,
            accept: None,
        })
    );
}

#[test]
fn one_check_is_asked_for_by_the_name_its_finding_carries() {
    assert_eq!(
        command(DIAGNOSE, disturbing(Some(KILLSWITCH))),
        Some(Command::Doctor {
            narrowing: Narrowing::Check(KILLSWITCH.to_owned()),
            disruptive: true,
            accept: None,
        })
    );
}

#[test]
fn a_narrowing_that_names_nothing_is_refused_rather_than_run_as_the_whole_suite() {
    // A name lemonfiber does not know is a mistake to correct rather than a request
    // to answer with everything — and here answering with everything would mean
    // disturbing everything, which is the worst version of that mistake there is.
    assert_eq!(
        refusal(DIAGNOSE, disturbing(Some("nonsense"))),
        Some(Refused::Unrecognised {
            argument: "only".to_owned(),
            offered: "try a group such as vpn, or one check by the name a finding gives it, \
                      such as vpn.killswitch"
                .to_owned(),
        })
    );
}

// ── It cannot be asked for as the read it is not ──────────────────────────────

#[test]
fn a_run_that_disturbs_nothing_is_refused_rather_than_read_as_a_diagnosis() {
    // Without the widening this would be `/api/checks`, which is served. Two ways to
    // ask one thing is the arrangement every read on this surface is kept out of, and
    // the refusal names the field rather than the endpoint, the way `accept` names
    // the check it was not given.
    assert_eq!(
        refusal(DIAGNOSE, Arguments::default()),
        Some(Refused::Missing {
            action: DIAGNOSE.to_owned(),
            argument: "disruptive".to_owned(),
        })
    );
}

#[test]
fn a_narrowed_run_that_disturbs_nothing_is_refused_for_the_widening_it_lacks() {
    // The narrowing alone is `/api/checks?only=…`, which is served too. So a request
    // that named a group and left the widening out is refused for the widening
    // rather than carried out as the read it happens to match.
    assert_eq!(
        refusal(
            DIAGNOSE,
            Arguments {
                only: Some("vpn".to_owned()),
                ..Arguments::default()
            }
        ),
        Some(Refused::Missing {
            action: DIAGNOSE.to_owned(),
            argument: "disruptive".to_owned(),
        })
    );
}

#[test]
fn a_diagnosis_answers_no_warning_and_agrees_to_no_repair() {
    // Three decisions, three requests. Widening the suite is not accepting what it
    // warns about and not consenting to what it offers to put right, so the two
    // arguments that carry those are refused here rather than dropped.
    for argument in ["check", "confirm"] {
        let given = match argument {
            "check" => Arguments {
                check: Some(KILLSWITCH.to_owned()),
                ..disturbing(None)
            },
            _ => Arguments {
                confirm: true,
                ..disturbing(None)
            },
        };
        assert_eq!(
            refusal(DIAGNOSE, given),
            Some(Refused::Unwanted {
                action: DIAGNOSE.to_owned(),
                argument: argument.to_owned(),
            })
        );
    }
}

// ── It runs for as long as it disturbs, so it is not waited for ───────────────

#[test]
fn a_disturbing_run_is_a_wait_an_operator_can_watch() {
    // It reaches the container engine to drop the tunnel and the services to search
    // for releases, and it is bounded rather than quick: a request that waited for it
    // would tie the disturbance to one connection, and a browser that closed the tab
    // would leave the tunnel down.
    let Some(command) = command(DIAGNOSE, disturbing(None)) else {
        unreachable!("this names an action this surface offers");
    };
    assert_eq!(answering(&command), Answering::Later);
}

// ── The route itself, driven without a socket ─────────────────────────────────

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
        bound: lemonfiber_api::guard::Binding::here(8472),
        admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
        jobs: Jobs::default(),
        live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
    })
}

/// What the route answered, as the status it answered under and what it said.
async fn said(body: &str) -> (u16, String) {
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/actions/{DIAGNOSE}"))
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
async fn a_disturbing_run_is_answered_with_a_name_for_the_work() {
    let (status, answered) = said(r#"{"disruptive":true}"#).await;
    assert_eq!(status, StatusCode::ACCEPTED.as_u16(), "{answered}");
    assert!(answered.contains(r#""kind":"job""#), "{answered}");
    assert!(
        answered.contains(&format!(r#""action":"{DIAGNOSE}""#)),
        "{answered}"
    );
}

#[tokio::test]
async fn a_narrowed_disturbing_run_is_answered_the_same_way() {
    let (status, answered) = said(&format!(r#"{{"disruptive":true,"only":"{KILLSWITCH}"}}"#)).await;
    assert_eq!(status, StatusCode::ACCEPTED.as_u16(), "{answered}");
    assert!(answered.contains(r#""kind":"job""#), "{answered}");
}

#[tokio::test]
async fn asking_for_it_without_the_widening_is_refused_over_the_wire_too() {
    let (status, answered) = said("{}").await;
    assert_eq!(status, StatusCode::BAD_REQUEST.as_u16());
    assert!(answered.contains("needs `disruptive`"), "{answered}");
}

#[tokio::test]
async fn the_flag_the_command_line_spells_is_not_a_name_this_carrier_holds() {
    // `--only` and `--disruptive` are the flags; `only` and `disruptive` are the
    // fields. A caller that sent `disruptive_checks` is told, rather than having a
    // whole request quietly mean the plain diagnosis.
    let (status, _) = said(r#"{"disruptive_checks":true}"#).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY.as_u16());
}
