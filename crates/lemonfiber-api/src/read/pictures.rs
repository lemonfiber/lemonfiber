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
use lemonfiber_core::app::{Command, Viewing, Whom};
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
    let (member, id) = match asked_for(&caller, read, id, query.as_deref()) {
        Ok(asked) => asked,
        Err(refused) => return *refused,
    };
    if let Some(asker) = caller.member() {
        spoke(&serving.ctx, asker);
    }
    match picture(&serving.ctx, &member, &id, which).await {
        Ok(pictured) => shown(pictured),
        Err(problem) => went_wrong(&problem),
    }
}

/// Whose shelf and which title a picture read asks for, ruled on as the title read is,
/// or what the caller is answered with instead.
fn asked_for(
    caller: &Caller,
    read: &str,
    id: String,
    query: Option<&str>,
) -> Result<(Whom, String), Box<Response>> {
    let given = wanted(read, query).map_err(|problem| Box::new(went_wrong(&problem)))?;
    let asked = Wanted {
        title: Some(id),
        ..given
    };
    let command = named(TITLE, asked).map_err(|why| Box::new(why.answered()))?;
    titled(may(caller, Door::Reading, command).granted())
}

/// Whose shelf and which title a granted title read names, or what the caller is
/// answered with where the ruling granted no title read.
fn titled(granted: Result<Command, Box<Response>>) -> Result<(Whom, String), Box<Response>> {
    match granted {
        Ok(Command::Viewing(Viewing::Title { member, id })) => Ok((member, id)),
        Ok(_) => Err(Box::new(Refusal::NoSuchRead.answered())),
        Err(refused) => Err(refused),
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
