//! The two reads about lemonfiber's own files, for a caller with no filesystem.
//!
//! What a shell gets from the directory itself: which backups are here to restore
//! from, and the support bundle it just asked to be made. Both are answers to the
//! same shortcoming — the command line names a path because there is one in front
//! of whoever typed it, and a browser has none to name, so what a path would have
//! given it is given by the server instead.
//!
//! **A name, never a path.** The listing is names, a restore is asked for by name,
//! and a bundle is asked for by name. Every one of them is resolved beneath a
//! directory lemonfiber chose, by the core, so nothing this surface hands over is a
//! path the server can read. The server runs as the operator: a path it accepted
//! from a request would be a path it would follow.
//!
//! **The bundle is handed over, not described.** It answers with a file rather than
//! with an envelope, for the reason `/api/logs` answers with a document per line:
//! what was asked for is not a value. It is offered no more widely than the
//! bundle's own request is — the same token, carried in the same header, so it is
//! fetched rather than linked; the same loopback; and a directory only a bundle
//! asked for through this surface is ever written into.

use axum::body::Body;
use axum::extract::{Path, RawQuery, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use lemonfiber_core::app::{dispatch, Command, Fetching, Outcome, Surfacing};
use lemonfiber_core::model::Octets;

use crate::admission::Caller;
use crate::read::table::{wanted, BUNDLE};
use crate::router::Serving;
use crate::routing::Serves;
use crate::serve::{carrying, operator_only};

use super::went_wrong;

/// That the answer is a file to keep rather than a page to show.
///
/// Without a filename of its own. The name is in the address the browser asked at,
/// which it already has and which this surface did not have to quote into a header.
const KEEP_IT: &str = "attachment";

/// The reads about the files lemonfiber keeps.
pub(super) fn routes() -> Serves<Serving> {
    Serves::new().get(BUNDLE, bundle, &[])
}

/// One support bundle this run kept, handed over whole.
///
/// The browser's answer to `--out`. A bundle asked for here is written with
/// lemonfiber's own files and then handed to whoever asked for it, which is what a
/// surface with no path to name has instead of one — and it works where naming a
/// path would not, because the machine running lemonfiber need not be the machine
/// the browser is on.
///
/// Which file is decided by the core, from the name and the directory it keeps
/// bundles in. This does not build a path and could not: what arrives here is text,
/// and text that reached the platform as a path would reach the whole filesystem
/// the server runs with. The query string goes through the same door as every other
/// read's, which for this one means there is no parameter it will take.
async fn bundle(
    State(serving): State<Serving>,
    caller: Caller,
    Path(name): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    if let Some(refusal) = operator_only(&caller) {
        return refusal;
    }
    if let Err(problem) = wanted(BUNDLE, query.as_deref()) {
        return went_wrong(&problem);
    }
    match dispatch(
        Command::Surface(Surfacing::Fetch(Fetching::Bundle { name })),
        &serving.ctx,
    )
    .await
    .and_then(Outcome::octets)
    {
        Ok(bundle) => handed_over(bundle),
        Err(problem) => went_wrong(&problem),
    }
}

/// A bundle as a file a browser keeps, rather than as a page it shows.
fn handed_over(bundle: Octets) -> Response {
    let mut response = carrying(StatusCode::OK, bundle.media_type, Body::from(bundle.bytes));
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static(KEEP_IT),
    );
    response
}
