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

use crate::actions::Jobs;
use crate::events::Streaming;
use crate::guard::Token;
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
}

/// Every endpoint this surface answers, behind the guard they share.
///
/// Routes are merged before the layer is applied, so the guard wraps the whole
/// tree — a path nothing serves is refused rather than reported as missing, and
/// which paths exist is not something an unauthenticated caller can map.
///
/// The stream is merged here too, after the state the rest share and before the
/// layer, even though it carries its own state and checks admission itself. Its
/// own check is what makes it safe today; being inside the layer is what makes
/// the next route added beside it guarded by having been added.
pub fn routes(serving: Serving, streaming: Arc<Streaming>) -> Router {
    let endpoints = Router::new()
        .merge(crate::read::routes())
        .merge(crate::actions::routes())
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
