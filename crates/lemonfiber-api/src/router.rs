//! Where the endpoints are put together, and what every one of them meets first.
//!
//! One router, so a request reaching any endpoint has already passed the same
//! guard. The check is a layer over the whole tree rather than a line each
//! handler remembers to write, which means an endpoint added later is guarded by
//! having been added rather than by whoever added it having read this.
//!
//! What each part of the surface answers is declared beside that part. This only
//! assembles them, and holds the one thing all of them are handed.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::Router;
use lemonfiber_core::app::Ctx;

use crate::events::live::Live;
use crate::events::Streaming;
use crate::guard::Token;
use crate::jobs::Jobs;
use crate::serve::{admitted, refused};

/// What every handler is given.
///
/// The context is the same one the command line runs against, so an endpoint
/// reaches the outside world through the ports a command does rather than
/// through anything of its own.
#[derive(Clone)]
pub struct Serving {
    /// The world a command runs against.
    pub ctx: Arc<Ctx>,
    /// The secret minted for this run, which every request must carry.
    pub token: Arc<Token>,
    /// The address this server is listening on, which a request must name.
    pub bound: SocketAddr,
    /// The work this run started, which the actions that take minutes are named
    /// and left to run under.
    pub jobs: Jobs,
    /// The one stream, for the work whose output arrives while it runs.
    ///
    /// The same stream the events route serves from its own state, shared rather
    /// than gathered again: what an endpoint needs here is the ability to *say*,
    /// which is what a follow's lines and a wait's words both need, and two
    /// streams would be two orders for one run of events.
    pub live: Arc<Live>,
}

/// Every endpoint this surface answers, behind the guard they share.
///
/// Routes are merged before the layer is applied, so the guard wraps every route
/// declared here: an endpoint added beside the others is guarded by having been
/// added, rather than by anyone remembering to wrap it.
///
/// What it does **not** wrap is a path under `/api/` that no route here declares.
/// This router is merged under the app's fallback, and axum keeps one fallback per
/// tree, so an unmatched path is answered by [`crate::frontend::page`] — which
/// refuses to hand a page to anything under `/api/` and returns a plain absence
/// instead. That absence is deliberate and argued where it is written; the cost of
/// it is that an unauthenticated caller can tell a path that exists from one that
/// does not, since the first answers `403` and the second `404`. The endpoints are
/// named in the contract every SDK is generated from, so what that reveals is
/// already published — but it is not nothing, and this said the opposite until a
/// test asked the composed surface rather than this function.
///
/// The stream is merged here too, after the state the rest share and before the
/// layer, even though it carries its own state and checks admission itself. Its
/// own check is what makes it safe today; being inside the layer is what makes
/// the next route added beside it guarded by having been added.
pub fn routes(serving: Serving, streaming: Arc<Streaming>) -> Router {
    let endpoints = Router::new()
        .merge(crate::read::routes())
        .merge(crate::actions::routes())
        .merge(crate::jobs::routes())
        .merge(crate::setup::routes())
        .with_state(serving.clone())
        .merge(crate::events::routes(streaming));
    endpoints.layer(middleware::from_fn_with_state(serving, guarded))
}

/// Let a request through, or turn it away before a handler runs.
async fn guarded(State(serving): State<Serving>, request: Request, next: Next) -> Response {
    match admitted(request.headers(), &serving.token, serving.bound) {
        Ok(()) => next.run(request).await,
        Err(refusal) => refused(refusal),
    }
}
