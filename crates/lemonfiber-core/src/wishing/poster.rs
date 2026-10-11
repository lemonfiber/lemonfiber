//! The poster of a title a member's search found, fetched by the core and handed to
//! that member alone.
//!
//! The member's client never reaches the catalogue: the core fetches the picture from
//! the location the request service gave for that title, and only where that location
//! lies beneath the one catalogue address the account of what leaves this machine
//! names. The host is resolved and held to addresses out on the internet before
//! anything is sent, nothing follows a redirect, and only a raster image of at most
//! [`PICTURE_MOST`] bytes is passed on.

use url::Url;

use super::{no_such_title, not_working, reached, refused, titled};
use crate::app::Ctx;
use crate::config::REACH_POSTERS_KEY;
use crate::error::codes::wish::NO_PICTURE;
use crate::error::Problem;
use crate::outbound::POSTERS;
use crate::outward::checked;
use crate::ports::http::{Method, Request};
use crate::ports::media::Kind;
use crate::ports::service::{Image, PICTURE_MOST};
use crate::schemes::SECURE_SCHEME;
use crate::screening::{passed_on, Pictured};

/// The poster of the title `id` of `kind`, as written, for the member `named`.
///
/// # Errors
///
/// A [`Problem`] wherever the member may not ask for the title, as [`titled`] judges;
/// where the title has no poster, the operator switched posters off, or what came back
/// is not a raster image of at most [`PICTURE_MOST`] bytes; and where the location is not
/// beneath the catalogue's address, stands for somewhere that is not out on the internet,
/// or does not answer.
pub async fn poster(
    ctx: &Ctx,
    named: &str,
    kind: &str,
    id: &str,
) -> Result<Pictured, Box<Problem>> {
    let reached = reached(ctx, named).await?;
    let kind = Kind::for_media_type(kind).ok_or_else(no_such_title)?;
    let detail = titled(&reached, kind, id).await?;
    if !ctx.settings.reaching.allows(REACH_POSTERS_KEY) {
        return Err(no_picture());
    }
    let location = detail.poster.ok_or_else(no_picture)?;
    let request = beneath_the_catalogue(&location).ok_or_else(not_working)?;
    let request = checked(ctx.seams.resolver.as_ref(), request)
        .await
        .map_err(|_| not_working())?;
    let fetched = ctx
        .seams
        .http
        .fetch(&request, PICTURE_MOST)
        .await
        .map_err(|_| not_working())?;
    if !fetched.is_success() {
        return Err(no_picture());
    }
    let media_type = fetched
        .header("content-type")
        .unwrap_or_default()
        .to_owned();
    fetched
        .bytes
        .map(|bytes| Image { media_type, bytes })
        .and_then(passed_on)
        .ok_or_else(no_picture)
}

/// The request for `location`, where it lies beneath [`POSTERS`]: over https, on the same
/// host and port, with no credentials, and at a path below the catalogue's own.
fn beneath_the_catalogue(location: &str) -> Option<Request> {
    let given = Url::parse(location).ok()?;
    let catalogue = Url::parse(POSTERS).ok()?;
    let below = format!("{}/", catalogue.path());
    let beneath = given.scheme() == SECURE_SCHEME
        && given.scheme() == catalogue.scheme()
        && given.host_str() == catalogue.host_str()
        && given.port_or_known_default() == catalogue.port_or_known_default()
        && given.username().is_empty()
        && given.password().is_none()
        && given.path().starts_with(&below);
    beneath.then(|| Request {
        method: Method::Get,
        url: given.into(),
        headers: Vec::new(),
        body: None,
        pinned: None,
    })
}

/// The refusal for a title with no picture to show.
fn no_picture() -> Box<Problem> {
    refused(NO_PICTURE, "There's no picture of that title to show.")
}

#[cfg(test)]
mod tests;
