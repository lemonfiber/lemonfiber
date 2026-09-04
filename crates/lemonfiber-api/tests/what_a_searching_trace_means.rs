//! The one trace a browser asks for by changing something.
//!
//! Every other trace on this surface is a `GET`: `/api/trace` follows an item across
//! the services and touches nothing. Widened to asking the indexers what they carry
//! for it, the same request stops being a read — the search is real and is counted
//! towards the daily allowance the indexers hold the operator to. So it is asked for
//! at the door changes are asked for, which is the only door this surface has that is
//! not a read.
//!
//! What it buys is the distinction the read cannot draw. An item somebody asked for
//! that nothing has been grabbed for stopped for one of two reasons, and no service in
//! this stack can tell them apart from its own records: the indexers carry nothing for
//! it, or they carry releases the quality in force rejects. Only the second is
//! something the operator can end by choosing differently, so a trace that reported
//! both as the same silence would be hiding the one with a remedy behind the one
//! without.
//!
//! What that leaves this file to hold still is three things.
//!
//! **It is the same request the command line makes.** `lemonfiber trace … --search`
//! reaches `Command::Trace` with the searching set, and so does this; nothing new was
//! invented in the core for a browser, and nothing here assembles a command of its
//! own.
//!
//! **It cannot impersonate the read.** Asked without the widening it would be the
//! plain trace, which is already served — so the widening is required rather than
//! defaulted, and a request that leaves it out is refused by name.
//!
//! **It can be narrowed the way the read is narrowed.** A trace is asked about a show
//! and, where somebody wants it, about one season of that show. Both travel, or a
//! browser asking where one season is would spend the search on a report about every
//! season there is.
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
use lemonfiber_core::platform::Environment;
use lemonfiber_fixtures::ports::{Chance, Idle, Stopped};

/// The name the action is asked for under.
///
/// Not `trace`, which is the read. One word answering at two doors is the arrangement
/// every read on this surface is kept out of.
const SEARCH: &str = "search";

/// What is being followed, as somebody would say it.
const FOLLOWED: &str = "The Expanse";

/// A searching run, as the fields a request carries.
fn searching(season: Option<u32>) -> Arguments {
    Arguments {
        term: Some(FOLLOWED.to_owned()),
        season,
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
fn a_searching_trace_reaches_the_command_the_command_line_reaches() {
    // `lemonfiber trace "The Expanse" --search`, over every season: the widening is
    // the whole of what this asks for beyond what the read already takes.
    assert_eq!(
        command(SEARCH, searching(None)),
        Some(Command::Trace {
            term: FOLLOWED.to_owned(),
            season: None,
            searching: true,
        })
    );
}

#[test]
fn the_season_the_reading_was_narrowed_to_narrows_the_search_too() {
    // `lemonfiber trace "The Expanse" --season 2 --search`. Dropped, the search would
    // be spent on a report about every season of a show somebody asked about one of.
    assert_eq!(
        command(SEARCH, searching(Some(2))),
        Some(Command::Trace {
            term: FOLLOWED.to_owned(),
            season: Some(2),
            searching: true,
        })
    );
}

// ── It cannot be asked for as the read it is not ──────────────────────────────

#[test]
fn a_trace_that_asks_the_indexers_nothing_is_refused_rather_than_read_as_a_trace() {
    // Without the widening this would be `/api/trace?term=…`, which is served. So a
    // request that named a show and left the widening out is refused for the widening
    // rather than carried out as the read it happens to match.
    assert_eq!(
        refusal(
            SEARCH,
            Arguments {
                term: Some(FOLLOWED.to_owned()),
                ..Arguments::default()
            }
        ),
        Some(Refused::Missing {
            action: SEARCH.to_owned(),
            argument: "disruptive".to_owned(),
        })
    );
}

#[test]
fn a_search_with_nothing_to_follow_is_refused_rather_than_run_over_everything() {
    // A trace with no subject follows nothing, and there is no whole-library reading
    // for it to fall back to — so it is refused by name, as the read refuses the same
    // omission.
    assert_eq!(
        refusal(
            SEARCH,
            Arguments {
                disruptive: true.into(),
                ..Arguments::default()
            }
        ),
        Some(Refused::Missing {
            action: SEARCH.to_owned(),
            argument: "term".to_owned(),
        })
    );
}

#[test]
fn a_blank_show_is_nothing_named_rather_than_a_show_with_no_name() {
    // A browser that sent the field and left it alone asks the same thing as one that
    // left it out, and is answered in the same sentence.
    assert_eq!(
        refusal(
            SEARCH,
            Arguments {
                term: Some("   ".to_owned()),
                disruptive: true.into(),
                ..Arguments::default()
            }
        ),
        Some(Refused::Missing {
            action: SEARCH.to_owned(),
            argument: "term".to_owned(),
        })
    );
}

#[test]
fn a_search_agrees_to_nothing_and_narrows_no_diagnosis() {
    // Different requests, different arguments. Asking the indexers what they carry is
    // not agreeing to a cost and not narrowing a suite of checks, so the two that
    // carry those are refused here rather than dropped.
    for argument in ["only", "confirm"] {
        let given = match argument {
            "only" => Arguments {
                only: Some("vpn".to_owned()),
                ..searching(None)
            },
            _ => Arguments {
                confirm: true,
                ..searching(None)
            },
        };
        assert_eq!(
            refusal(SEARCH, given),
            Some(Refused::Unwanted {
                action: SEARCH.to_owned(),
                argument: argument.to_owned(),
            })
        );
    }
}

#[test]
fn the_read_this_widens_is_not_an_action_by_its_own_name() {
    // `trace` stays the read. A surface offering it at both doors would be two ways to
    // ask one thing, which is what the second name exists to prevent.
    assert!(matches!(
        refusal("trace", searching(None)),
        Some(Refused::Unknown { .. })
    ));
}

// ── It reaches the indexers, so it is not waited for ──────────────────────────

#[test]
fn a_searching_trace_is_a_wait_an_operator_can_watch() {
    // It reaches the services and through them the indexers, and it is bounded rather
    // than quick: a request that waited for it would tie the search to one connection.
    let Some(command) = command(SEARCH, searching(None)) else {
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
        .uri(format!("/api/actions/{SEARCH}"))
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
async fn a_searching_run_is_answered_with_a_name_for_the_work() {
    let (status, answered) = said(r#"{"disruptive":true,"term":"The Expanse"}"#).await;
    assert_eq!(status, StatusCode::ACCEPTED.as_u16(), "{answered}");
    assert!(answered.contains(r#""kind":"job""#), "{answered}");
    assert!(
        answered.contains(&format!(r#""action":"{SEARCH}""#)),
        "{answered}"
    );
}

#[tokio::test]
async fn a_run_narrowed_to_one_season_is_answered_the_same_way() {
    let (status, answered) = said(r#"{"disruptive":true,"term":"The Expanse","season":2}"#).await;
    assert_eq!(status, StatusCode::ACCEPTED.as_u16(), "{answered}");
    assert!(answered.contains(r#""kind":"job""#), "{answered}");
}

#[tokio::test]
async fn asking_for_it_without_the_widening_is_refused_over_the_wire_too() {
    let (status, answered) = said(r#"{"term":"The Expanse"}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST.as_u16());
    assert!(answered.contains("needs `disruptive`"), "{answered}");
}

#[tokio::test]
async fn the_flag_the_command_line_spells_is_not_a_name_this_carrier_holds() {
    // `--search` is the flag; `disruptive` is the field, because what it asks for is
    // the same thing the diagnosis asks for under the same word. A caller that sent
    // the flag's own spelling is told, rather than having a whole request quietly
    // mean the plain trace.
    let (status, _) = said(r#"{"search":true,"term":"The Expanse"}"#).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY.as_u16());
}
