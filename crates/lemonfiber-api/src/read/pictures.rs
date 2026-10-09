//! A title's pictures, for a browser.
//!
//! A browser can neither present a session to the guarded front door nor pin its
//! certificate, so it reads a title's pictures here with its own token. Each is read as
//! the member whose shelf holds the title, under the same ruling the title read is.

use axum::body::Body;
use axum::extract::{Path, RawQuery, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use lemonfiber_core::app::{Command, Viewing};
use lemonfiber_core::ports::service::Picture;
use lemonfiber_core::screening::{picture, spoke, Pictured};

use crate::admission::Caller;
use crate::entitled::{may, Door};
use crate::read::table::{named, wanted, Wanted, BACKDROP, POSTER, TITLE};
use crate::refusal::Refusal;
use crate::router::Serving;
use crate::serve::carrying;

use super::went_wrong;

/// That a picture is the asker's own to keep, and nobody else's.
const PRIVATE: &str = "private";

/// The two pictures a title has.
pub(super) fn routes() -> Router<Serving> {
    Router::new()
        .route(
            POSTER,
            get(|state, caller, id, query| pictured(state, caller, id, query, Picture::Poster)),
        )
        .route(
            BACKDROP,
            get(|state, caller, id, query| pictured(state, caller, id, query, Picture::Backdrop)),
        )
}

/// One of a title's pictures, read as the member the title read would be read as.
async fn pictured(
    State(serving): State<Serving>,
    caller: Caller,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
    which: Picture,
) -> Response {
    let read = match which {
        Picture::Poster => POSTER,
        Picture::Backdrop => BACKDROP,
    };
    let given = match wanted(read, query.as_deref()) {
        Ok(given) => given,
        Err(problem) => return went_wrong(&problem),
    };
    let asked = Wanted {
        title: Some(id),
        ..given
    };
    let command = match named(TITLE, asked) {
        Ok(command) => command,
        Err(why) => return why.answered(),
    };
    let granted = may(&caller, Door::Reading, command).granted();
    let Ok(Command::Viewing(Viewing::Title { member, id })) = granted else {
        return granted
            .err()
            .map_or_else(|| Refusal::NoSuchRead.answered(), |refused| *refused);
    };
    if let Some(asker) = caller.member() {
        spoke(&serving.ctx, asker);
    }
    match picture(&serving.ctx, &member, &id, which).await {
        Ok(pictured) => shown(pictured),
        Err(problem) => went_wrong(&problem),
    }
}

/// A picture as a browser draws it, kept by that browser alone.
fn shown(pictured: Pictured) -> Response {
    let mut response = carrying(
        StatusCode::OK,
        pictured.media_type,
        Body::from(pictured.bytes),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(PRIVATE));
    response
}

#[cfg(test)]
mod tests;
