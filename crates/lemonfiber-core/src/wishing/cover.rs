//! The cover of music or a book, fetched by the core and handed to the member alone: of
//! a candidate their search found, and of an ask the core keeps after that search is
//! gone.
//!
//! A cover is wherever the curator says it is, so it is fetched from any host, but only
//! over https with no credentials, from a host every address of which is out on the
//! internet, with no redirect followed. Each host is noted before it is reached, so the
//! account of what leaves this machine names every one.

use url::Url;

use super::collect::curator_of;
use super::poster::{handed, no_picture, outward};
use super::{no_such_title, not_working, reached};
use crate::app::Ctx;
use crate::collected::{Reference, Unit};
use crate::config::REACH_POSTERS_KEY;
use crate::error::Problem;
use crate::ports::http::{Method, Request};
use crate::schemes::SECURE_SCHEME;
use crate::screening::Pictured;

/// The cover of the `unit` the curator's `reference` names, for the member `named`.
///
/// # Errors
///
/// A [`Problem`] wherever the member may not search, as [`reached`] judges; where no
/// curator here answers for the unit's medium or it holds no such candidate; and where
/// the cover cannot be fetched as [`fetched_cover`] says.
pub async fn cover(
    ctx: &Ctx,
    named: &str,
    unit: Unit,
    reference: &str,
) -> Result<Pictured, Box<Problem>> {
    reached(ctx, named).await?;
    let (_, curator) = curator_of(ctx, unit).await?;
    let candidate = curator
        .candidate(unit, reference)
        .await
        .map_err(|_| not_working())?
        .ok_or_else(no_such_title)?;
    fetched_cover(ctx, candidate.cover.as_deref()).await
}

/// The cover of the ask the core keeps under `reference`, for the member `named` where
/// one is named, who must be the one who asked, and for whoever runs the house where
/// none is.
///
/// # Errors
///
/// A [`Problem`] where no kept ask goes by that reference or the member named did not
/// make it, wherever [`reached`] refuses the member named, and where the cover cannot be
/// fetched as [`fetched_cover`] says.
pub async fn kept_cover(
    ctx: &Ctx,
    named: Option<&str>,
    reference: &str,
) -> Result<Pictured, Box<Problem>> {
    let named_as: Reference = reference.parse().map_err(|_| no_such_title())?;
    let ask = crate::collected::run::every(ctx)
        .into_iter()
        .find(|ask| ask.named() == named_as)
        .ok_or_else(no_such_title)?;
    if let Some(named) = named {
        let reached = reached(ctx, named).await?;
        if reached.member.id != ask.member {
            return Err(no_such_title());
        }
    }
    fetched_cover(ctx, ask.cover.as_deref()).await
}

/// The cover at `location`, its host noted once it is shown to be out on the internet and
/// before it is reached.
///
/// # Errors
///
/// A [`Problem`] where there is no cover, the operator switched posters off, or what
/// came back is not a picture [`handed`] passes on; and where the location is not https,
/// carries credentials, cannot be noted, or stands for somewhere that is not out on the
/// internet.
async fn fetched_cover(ctx: &Ctx, location: Option<&str>) -> Result<Pictured, Box<Problem>> {
    if !ctx.settings.reaching.allows(REACH_POSTERS_KEY) {
        return Err(no_picture());
    }
    let location = location.ok_or_else(no_picture)?;
    let (host, request) = public(location).ok_or_else(not_working)?;
    let request = outward(ctx, request).await?;
    crate::outbound::covers::record(ctx, &host).ok_or_else(not_working)?;
    handed(ctx, request).await
}

/// The host `location` names and the request for it, where it is https with no
/// credentials.
fn public(location: &str) -> Option<(String, Request)> {
    let given = Url::parse(location).ok()?;
    let plain = given.scheme() == SECURE_SCHEME
        && given.username().is_empty()
        && given.password().is_none();
    let host = given.host_str().filter(|_| plain)?.to_owned();
    Some((
        host,
        Request {
            method: Method::Get,
            url: given.into(),
            headers: Vec::new(),
            body: None,
            pinned: None,
            deadline: None,
        },
    ))
}
