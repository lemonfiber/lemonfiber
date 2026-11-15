//! The live TV guide for one member: each channel they may watch now, with the
//! programmes on it from now across the guide window.
//!
//! **The channels are the ones live TV answers.** Which channels a member may watch is
//! asked as it is for the channels list, so the guide never offers a channel the list
//! leaves out. Each programme in the guide is read against the member's limits too, and
//! one outside them is left out as a gap.
//!
//! **The window is the operator's.** It is the hours `LEMONFIBER_GUIDE_HOURS` holds,
//! and each channel says where the media server's guide ends inside it, so a window
//! longer than the guide reaches no further than the guide does.

use crate::app::targets::Media;
use crate::app::Ctx;
use crate::error::Problem;
use crate::instant::{Instant, HOUR};
use crate::model::{GuideReport, Guided};
use crate::ports::service::{Airing, Channel, Failure, Listing, Member};
use crate::rating::Limits;

use super::{admitted, limits, member, reachable, server, unanswered};

/// The live TV guide `named` may watch: each channel with its programmes from now
/// across the guide window.
///
/// # Errors
///
/// A [`Problem`] where there is no server, the member is not in the household, or the
/// server does not answer.
pub(in crate::screening) async fn guide(
    ctx: &Ctx,
    named: &str,
) -> Result<GuideReport, Box<Problem>> {
    let server = server(ctx).await?;
    let member = member(server.identity.as_ref(), named).await?;
    let channels = reachable(&server, &member).await?;
    let limits = limits(&server, &member).await?;
    let channels = admitted(&limits, channels);
    let hours = crate::settings::guide_hours(ctx);
    let listings = if channels.is_empty() {
        Vec::new()
    } else {
        listed(&server, &member, hours).await?
    };
    let now = Instant::from_seconds(ctx.seconds());
    let until = Instant::from_seconds(ctx.seconds().saturating_add(u64::from(hours) * HOUR));
    Ok(GuideReport {
        channels: channels
            .into_iter()
            .map(|channel| guided(channel, &listings, &limits, now, until))
            .collect(),
        member: member.name,
        id: member.id,
        rehearsed: false,
    })
}

/// Each channel's guide `member`'s own account may watch across `hours` from now; none
/// where the server has no live TV.
async fn listed(server: &Media, member: &Member, hours: u32) -> Result<Vec<Listing>, Box<Problem>> {
    match server.serve.guide(&member.id, hours).await {
        Ok(listings) => Ok(listings),
        Err(Failure::Unsupported { .. }) => Ok(Vec::new()),
        Err(_) => Err(unanswered()),
    }
}

/// `channel` with the programmes its listing holds airing between `now` and `until`, in
/// the order they air, each outside `limits` left out.
fn guided(
    channel: Channel,
    listings: &[Listing],
    limits: &Limits,
    now: Instant,
    until: Instant,
) -> Guided {
    let mut aired: Vec<&Airing> = listings
        .iter()
        .filter(|listing| listing.channel == channel.id)
        .flat_map(|listing| &listing.programmes)
        .filter(|airing| airing.ends > now && airing.starts < until)
        .collect();
    aired.sort_by_key(|airing| airing.starts);
    Guided {
        guide_until: aired.iter().map(|airing| airing.ends).max(),
        programmes: aired
            .into_iter()
            .filter(|airing| limits.admits(airing.certificate.as_deref()))
            .cloned()
            .collect(),
        id: channel.id,
        name: channel.name,
        number: channel.number,
    }
}
