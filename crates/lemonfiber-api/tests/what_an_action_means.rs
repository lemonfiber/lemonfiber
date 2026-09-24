//! What a write request asks for, and what it is refused for.
//!
//! The load-bearing property is not that any one action works. It is that the set
//! of them is the set the command line already has: every action reaches one of
//! the core's own commands, and a name that reaches none is refused rather than
//! invented. A surface that could grow an action of its own is a surface that has
//! started implementing behaviour, and this is where that is stopped.
//!
//! Which argument each action may be given is next door, in
//! `what_an_argument_reaches.rs`: it is a sweep over every action rather than a
//! reading of any one of them, and both halves are built from the same table of
//! what each action takes.
//!
//! Driven from outside the crate, because what a caller can reach is the thing
//! worth holding still.

mod acting;

use axum::body::to_bytes;
use axum::http::header;
use axum::Extension;
use lemonfiber_api::actions;
use lemonfiber_api::actions::{answering, named, Arguments, Refused};
use lemonfiber_api::admission::Caller;
use lemonfiber_api::events::live::Live;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::Command;
use lemonfiber_fixtures::ports::{Chance, Stopped};

/// Nothing named, which is what most actions are asked with.
fn nothing() -> Arguments {
    Arguments::default()
}

/// One form named, which is what most of the rest are asked with.
fn naming(form: &str) -> Arguments {
    Arguments {
        forms: vec![form.to_owned()],
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

use acting::*;

// ── Every action is one of the command line's own ─────────────────────────────

// ── Arguments mirror the flags the command takes ──────────────────────────────

// ── Which answers wait, and which are named and left to run ───────────────────

// ── The route itself, driven without a socket ─────────────────────────────────

/// The action routes as a run builds them, over a context a test chose.
///
/// Stated here rather than assembled with the rest of the surface: what an
/// action *means* is this file's business, and that a request reaches it only
/// with a token is the router's, proven where the router is.
fn routed(random: Chance) -> axum::Router {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    actions::routes()
        .with_state(Serving {
            ctx: Arc::new(ctx().with_random(Arc::new(random))),
            token: Arc::new(token),
            bound: lemonfiber_api::guard::Binding::here(8471),
            admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
            jobs: Jobs::default(),
            live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
        })
        // The subject the guard puts on every request it admits. Mounted here because a
        // test builds these routes without the layer that carries it, and a handler that
        // asks who is calling is answered by the guard in a run rather than by the route.
        .layer(Extension(Caller::Machine))
}

/// What the route answered, as the status it answered under and what it said.
async fn said(random: Chance, action: &str, body: &str) -> (u16, String) {
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/actions/{action}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body.to_owned()));
    let Ok(request) = request else {
        unreachable!("a request built from values that are already headers cannot fail");
    };
    let served = tower::ServiceExt::oneshot(routed(random), request)
        .await
        .ok();
    let Some(response) = served else {
        unreachable!("the router is infallible; its handlers answer rather than fail");
    };
    let status = response.status().as_u16();
    let read = to_bytes(response.into_body(), usize::MAX).await;
    let bytes = read.map(|bytes| bytes.to_vec()).unwrap_or_default();
    (status, String::from_utf8(bytes).unwrap_or_default())
}

mod answering;
mod arguments;
mod naming;
