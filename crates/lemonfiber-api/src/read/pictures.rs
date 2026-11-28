//! A title's pictures, for a browser.
//!
//! A browser can neither present a session to the guarded front door nor pin its
//! certificate, so it reads a title's pictures here with its own token. Each is read as
//! the member whose shelf holds the title, under the same ruling the title read is.
//!
//! The poster of a title a member's search found is read here too, under the ruling the
//! search read is: the core fetches it for that member, so their client is never sent to
//! the catalogue.

use axum::body::Body;
use axum::extract::{Path, RawQuery, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use lemonfiber_core::app::{
    dispatch, Command, Ctx, Fetching, Outcome, Surfacing, Viewing, Whom, Wishing,
};
use lemonfiber_core::collected::Unit;
use lemonfiber_core::model::Octets;
use lemonfiber_core::ports::service::Picture;

use crate::admission::Caller;
use crate::entitled::{may, Door};
use crate::read::table::{
    named, wanted, Wanted, ASKABLE_POSTER, BACKDROP, POSTER, REQUEST_POSTER, TITLE,
};
use crate::refusal::Refusal;
use crate::router::Serving;
use crate::routing::Serves;
use crate::serve::carrying;

use super::went_wrong;

/// That a picture is the asker's own to keep, and nobody else's.
const PRIVATE: &str = "private";

/// The two pictures a title has.
pub(super) fn routes() -> Serves<Serving> {
    Serves::new()
        .get(
            POSTER,
            |state, caller, id, query| pictured(state, caller, id, query, Picture::Poster),
            &[],
        )
        .get(
            BACKDROP,
            |state, caller, id, query| pictured(state, caller, id, query, Picture::Backdrop),
            &[],
        )
        .get(ASKABLE_POSTER, found, &[])
        .get(REQUEST_POSTER, kept, &[])
}

/// The poster of a title a member's search found, fetched by the core for that member.
async fn found(
    State(serving): State<Serving>,
    caller: Caller,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let (member, given) = match searched_for(&caller, ASKABLE_POSTER, query.as_deref()) {
        Ok(asked) => asked,
        Err(refused) => return *refused,
    };
    caller.spoke(&serving.ctx).await;
    let kind = given.kind.unwrap_or_default();
    let asked = match Unit::chosen(Some(&kind), given.unit.as_deref()) {
        Ok(Some(unit)) => Fetching::Cover { member, unit, id },
        Ok(None) => Fetching::Found { member, kind, id },
        Err(_) => return Refusal::UnrecognisedArgument.answered(),
    };
    answered(&serving.ctx, asked).await
}

/// The cover of an ask the core keeps, fetched by the core for the member who made it or
/// for whoever runs the house.
async fn kept(
    State(serving): State<Serving>,
    caller: Caller,
    Path(reference): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let member = match searched_for(&caller, REQUEST_POSTER, query.as_deref()) {
        Ok((member, _)) => member,
        Err(refused) => return *refused,
    };
    caller.spoke(&serving.ctx).await;
    answered(&serving.ctx, Fetching::Kept { member, reference }).await
}

/// Whose search a picture read at `read` is for and what else it was asked with, ruled on
/// as the search read is, or what the caller is answered with instead.
fn searched_for(
    caller: &Caller,
    read: &str,
    query: Option<&str>,
) -> Result<(String, Wanted), Box<Response>> {
    let given = wanted(read, query).map_err(|problem| Box::new(went_wrong(&problem)))?;
    let search = Command::Wish(Wishing::Find {
        member: given.member.clone().unwrap_or_default(),
        term: String::new(),
        after: None,
        unit: None,
    });
    let member = searcher(may(caller, Door::Reading, search).granted())?;
    Ok((member, given))
}

/// Whose search a granted search read names, or what the caller is answered with where
/// the ruling granted no search.
fn searcher(granted: Result<Command, Box<Response>>) -> Result<String, Box<Response>> {
    match granted {
        Ok(Command::Wish(Wishing::Find { member, .. })) => Ok(member),
        Ok(_) => Err(Box::new(Refusal::NoSuchRead.answered())),
        Err(refused) => Err(refused),
    }
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
    caller.spoke(&serving.ctx).await;
    answered(&serving.ctx, Fetching::Title { member, id, which }).await
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
        Ok(Command::Viewing(Viewing::Title { member, id, .. })) => Ok((member, id)),
        Ok(_) => Err(Box::new(Refusal::NoSuchRead.answered())),
        Err(refused) => Err(refused),
    }
}

/// The picture `asked` fetches through the dispatcher, or what went wrong fetching it.
async fn answered(ctx: &Ctx, asked: Fetching) -> Response {
    match dispatch(Command::Surface(Surfacing::Fetch(asked)), ctx)
        .await
        .and_then(Outcome::octets)
    {
        Ok(pictured) => shown(pictured),
        Err(problem) => went_wrong(&problem),
    }
}

/// A picture as a browser draws it, kept by that browser alone.
fn shown(pictured: Octets) -> Response {
    let mut response = carrying(
        StatusCode::OK,
        pictured.media_type,
        Body::from(pictured.bytes),
    );
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(PRIVATE));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(NOTHING_RUNS),
    );
    response
}

/// The policy a picture is served under: nothing it holds is loaded or run.
const NOTHING_RUNS: &str = "default-src 'none'";

#[cfg(test)]
mod tests;
