//! Watching a service's lines arrive, after the request that asked has been
//! answered.
//!
//! The scrollback is a read: it ends, and what it read is the answer. Following
//! does not end, so there is nothing to answer with — which is the same shape
//! every long-running request on this surface has, and it is given the same two
//! halves. A name for the work, redeemed at [`crate::jobs`] and released there
//! when the operator has seen enough; and the stream a browser already holds
//! open, which is where the lines themselves arrive.
//!
//! One stream rather than a second one for logs. A browser watching what a
//! service says is watching the dashboard beside it, and two connections carrying
//! the same envelopes under the same guard would be this module's neighbour
//! written twice — with its own backlog, its own beat and its own reconnection.
//! Worse, they could not be ordered against each other: an operator who sees a
//! service go unhealthy on one and reads its last words on the other has no way
//! to know which came first. Mixing costs a client nothing, because the event
//! name on the wire is the envelope's kind, so a browser that is not following
//! never registered for `log` and never sees one.
//!
//! Nothing is dropped here. Every line the service produces is said, and saying
//! waits for nobody — a chatty container must not be able to hold up the gather
//! or another action's narration. What is bounded is the far end: a browser that
//! reads more slowly than a service speaks falls behind the window the stream
//! carries and is let go, the rule already in force for every other event, and it
//! comes back saying where it got to. The lines still in the backlog it is given;
//! the ones that have aged out of it are gone, and it is told the record cannot be
//! completed rather than handed part of one.

use std::sync::Arc;

use axum::response::Response;
use lemonfiber_core::app::{logs, Ctx};
use lemonfiber_core::model::{kind, Envelope};
use lemonfiber_core::ports::docker::LogQuery;

use crate::actions::unnameable;
use crate::events::live::Live;
use crate::events::wire::{Nature, Rendered};
use crate::jobs::{accepted, Job, Lease, Standing};
use crate::router::Serving;

/// What a follow is named as, which is the request it answers.
///
/// The command line spells following as a flag on `logs` rather than as a request
/// of its own, so a caller redeeming the name is told the name is for `logs` —
/// not for a word only this surface uses.
const ASKED_FOR: &str = "logs";

/// Begin following, and answer with the name it can be stopped by.
///
/// The stream is opened inside the work rather than before it, so a stack that
/// cannot be read is reported under the name like any other failure, instead of
/// being two different answers to one request depending on how far it got.
pub(crate) async fn followed(
    serving: &Serving,
    forms: Vec<String>,
    services: Vec<String>,
    tail: u32,
) -> Response {
    let Some(job) = Job::mint(serving.ctx.random.as_ref()) else {
        return unnameable();
    };
    let (ctx, live) = (Arc::clone(&serving.ctx), Arc::clone(&serving.live));
    serving
        .jobs
        .begin(&job, ASKED_FOR, Lease::WhileAsked, async move {
            following(&ctx, &live, &forms, &services, tail).await
        })
        .await;
    accepted(&job, ASKED_FOR)
}

/// Say every line the services produce, until there are no more or this is ended.
///
/// A follow has no outcome. It ends because it was released, because the run
/// ended, or because the containers it was reading stopped having anything to
/// say — and none of those is a value. So it ends the way work released by name
/// ends, and a caller redeeming the name afterwards reads that it is over rather
/// than an answer it never had.
async fn following(
    ctx: &Ctx,
    live: &Live,
    forms: &[String],
    services: &[String],
    tail: u32,
) -> Standing {
    let query = LogQuery { tail, follow: true };
    let mut lines = match logs(ctx, forms, services, query).await {
        Ok(opened) => opened,
        Err(problem) => return Standing::failed(&problem),
    };
    while let Some(line) = lines.recv().await {
        // A record rather than state: a line skipped is a hole in what a service
        // said, not a figure that has been overtaken, so a client that was away is
        // given the ones it missed rather than only the newest.
        let said = Rendered::of(Nature::Record, &Envelope::new(kind::LOG, &line));
        live.say_if_rendered(said).await;
    }
    Standing::Ended
}
