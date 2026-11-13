//! Live TV for one member: the channels their own account may watch, and a channel's
//! live stream opened for them on passes, renewed and closed.
//!
//! **Both questions are asked every time.** Which channels a member's own account may
//! watch is the media server's answer, read as that member and never as the adapter's
//! credential; whether the programme on one now is within their limits is read against
//! the server's own certificates. A channel opened is opened only where both allow it,
//! and each renewal asks both again, so a programme that moves outside a member's limits
//! stops being renewed.
//!
//! **A pass is issued only for bytes the guard asks about.** Each path the server
//! answers is held to the live-stream paths it declared, and a path none covers is given
//! no location and no pass.

use crate::app::targets::Media;
use crate::app::{Asker, Ctx, Live};
use crate::error::codes::play::{NOT_ON_THEIR_SHELF, NOT_THEIR_STREAM, NO_LIVE_TV};
use crate::error::Problem;
use crate::model::{ChannelsReport, TunedReport};
use crate::pass::{Key, Watching, LIFETIME};
use crate::ports::service::{joined, Channel, Failure, Member};
use crate::rating::Limits;

use super::door::{self, told, Door};
use super::tuned::{self, Open};
use super::{member, refused, server, unanswered};

/// The live TV channels `named` may watch, each with the programme on it now.
///
/// # Errors
///
/// A [`Problem`] where there is no server, the member is not in the household, or the
/// server does not answer.
pub(super) async fn channels(ctx: &Ctx, named: &str) -> Result<ChannelsReport, Box<Problem>> {
    let server = server(ctx).await?;
    let member = member(server.identity.as_ref(), named).await?;
    let channels = watchable(&server, &member).await?;
    Ok(ChannelsReport {
        member: member.name,
        id: member.id,
        channels,
        rehearsed: false,
    })
}

/// What asking about one live stream for `named` comes to: a channel opened on passes,
/// new passes for one the core opened, or one closed.
///
/// # Errors
///
/// A [`Problem`] where there is no server, the member is not in the household, the
/// channel is not one they may watch now, the live stream named is not one opened for
/// them, or the server does not answer.
pub(super) async fn tuned(
    ctx: &Ctx,
    named: &str,
    live: Live,
    asker: Asker,
) -> Result<TunedReport, Box<Problem>> {
    let server = server(ctx).await?;
    let member = member(server.identity.as_ref(), named).await?;
    match live {
        Live::Open(channel) => opened(ctx, &server, &member, &channel, asker).await,
        Live::Renew(stream) => {
            let open = theirs(ctx, &member, &stream)?;
            renewed(ctx, &server, &member, open, asker).await
        }
        Live::Stop(stream) => {
            let open = theirs(ctx, &member, &stream)?;
            if !ctx.dry_run {
                tuned::closed(ctx, server.serve.as_ref(), |one| {
                    one.closes_by == open.closes_by
                })
                .await;
            }
            Ok(TunedReport {
                channel: open.channel,
                member: member.name,
                rehearsed: ctx.dry_run,
                ..TunedReport::default()
            })
        }
    }
}

/// The channels `member`'s own account may watch whose programme now is within their
/// limits; none where their account may not watch live TV or the server has none.
async fn watchable(server: &Media, member: &Member) -> Result<Vec<Channel>, Box<Problem>> {
    let channels = reachable(server, member).await?;
    admitted(server, member, channels).await
}

/// The channels `member`'s own account may watch; none where it may not watch live TV
/// or the server has none.
async fn reachable(server: &Media, member: &Member) -> Result<Vec<Channel>, Box<Problem>> {
    match server.serve.channels(&member.id).await {
        Ok(channels) => Ok(channels),
        Err(Failure::Unsupported { .. }) => Ok(Vec::new()),
        Err(_) => Err(unanswered()),
    }
}

/// Those of `channels` whose programme now is within `member`'s limits.
async fn admitted(
    server: &Media,
    member: &Member,
    channels: Vec<Channel>,
) -> Result<Vec<Channel>, Box<Problem>> {
    let limits = Limits::certified(server.identity.as_ref(), &member.access)
        .await
        .ok_or_else(unanswered)?;
    Ok(channels
        .into_iter()
        .filter(|channel| {
            limits.admits(
                channel
                    .now
                    .as_ref()
                    .and_then(|now| now.certificate.as_deref()),
            )
        })
        .collect())
}

/// Refused unless `channel` is one `member` may watch now.
async fn allowed(server: &Media, member: &Member, channel: &str) -> Result<(), Box<Problem>> {
    let reachable = reachable(server, member).await?;
    if reachable.is_empty() {
        return Err(no_live_tv());
    }
    let admitted = admitted(server, member, reachable).await?;
    if admitted.iter().any(|one| one.id == channel) {
        Ok(())
    } else {
        Err(refused(
            NOT_ON_THEIR_SHELF,
            "That channel is not one this member may watch now",
        ))
    }
}

/// The live stream `member` holds that names `stream`.
fn theirs(ctx: &Ctx, member: &Member, stream: &str) -> Result<Open, Box<Problem>> {
    tuned::theirs(ctx, &member.id, stream).ok_or_else(|| {
        refused(
            NOT_THEIR_STREAM,
            "That live stream is not one opened for this member",
        )
    })
}

/// Open `channel`'s live stream for `member`, and answer where it is served.
async fn opened(
    ctx: &Ctx,
    server: &Media,
    member: &Member,
    channel: &str,
    asker: Asker,
) -> Result<TunedReport, Box<Problem>> {
    allowed(server, member, channel).await?;
    let unlocated = |why| unlocated(channel, member, why);
    let ready = match ready(ctx, server, asker).await {
        Ok(ready) => ready,
        Err(why) => return Ok(unlocated(why)),
    };
    if ctx.dry_run {
        return Ok(rehearsed(channel, member, ready));
    }
    let opened = match server.serve.tune(&member.id, channel).await {
        Ok(Some(opened)) => opened,
        Ok(None) | Err(Failure::Unsupported { .. }) => return Err(no_live_tv()),
        Err(_) => return Err(unanswered()),
    };
    let issued = ctx.seconds().saturating_add(LIFETIME);
    let open = Open {
        member: member.id.clone(),
        channel: channel.to_owned(),
        closes_by: opened.closes_by,
        direct: covered(&ready.watching, &opened.direct),
        transcoded: covered(&ready.watching, &opened.transcoded),
        streams: Vec::new(),
        issued,
        lapses: issued,
    };
    let streams: Vec<String> = [&open.direct, &open.transcoded]
        .into_iter()
        .flatten()
        .filter_map(|path| named(&ready.watching, path))
        .collect();
    if streams.is_empty() {
        let _ = server.serve.untune(&open.closes_by).await;
        return Ok(unlocated(told(asker, UNCOVERED.to_owned())));
    }
    let open = Open { streams, ..open };
    let report = located(&ready, member, &open, issued);
    tuned::kept(ctx, open);
    Ok(report)
}

/// The passes the live stream `open` the core opened for `member` was answered with,
/// held good for another [`LIFETIME`], where the channel is still one they may watch now.
async fn renewed(
    ctx: &Ctx,
    server: &Media,
    member: &Member,
    open: Open,
    asker: Asker,
) -> Result<TunedReport, Box<Problem>> {
    allowed(server, member, &open.channel).await?;
    let ready = match ready(ctx, server, asker).await {
        Ok(ready) => ready,
        Err(why) => return Ok(unlocated(&open.channel, member, why)),
    };
    if ctx.dry_run {
        return Ok(rehearsed(&open.channel, member, ready));
    }
    let lapses = ctx.seconds().saturating_add(LIFETIME);
    let passes: Vec<String> = [&open.direct, &open.transcoded]
        .into_iter()
        .flatten()
        .filter_map(|path| passed(&ready, member, &open, path))
        .collect();
    if !crate::app::project::passing::renewed(ctx, &passes, &member.id, lapses) {
        return Ok(unlocated(
            &open.channel,
            member,
            told(asker, UNRENEWED.to_owned()),
        ));
    }
    tuned::renewed(ctx, &open.closes_by, lapses);
    Ok(located(&ready, member, &open, lapses))
}

/// Everything a live stream's location is built from.
struct Ready {
    /// The guarded door's address.
    base: String,
    /// SHA-256 over the certificate the door presents.
    fingerprint: String,
    /// The key passes are issued with.
    key: Key,
    /// The live-stream paths the media server declared, compiled.
    watching: Watching,
}

/// Everything a live stream's location is built from, or why nothing can be located, in
/// the words `asker` is told.
async fn ready(ctx: &Ctx, server: &Media, asker: Asker) -> Result<Ready, String> {
    let (base, fingerprint) = match door::standing(ctx, server.serve.as_ref(), asker).await {
        Door::At {
            base, fingerprint, ..
        } => (base, fingerprint),
        Door::Unknown(why) => return Err(why),
    };
    let key =
        crate::app::project::passing::key(ctx).ok_or_else(|| told(asker, KEYLESS.to_owned()))?;
    let declared = door::declared(server.serve.as_ref())
        .await
        .map_err(|why| told(asker, why))?;
    let watching = Watching::of(&crate::pass::told(&declared))
        .map_err(|_| told(asker, UNDECLARED.to_owned()))?;
    tuned::place(ctx).ok_or_else(|| told(asker, NOWHERE.to_owned()))?;
    Ok(Ready {
        base,
        fingerprint,
        key,
        watching,
    })
}

/// `path`, where a live-stream path covers it and names a live stream in it.
fn covered(watching: &Watching, path: &str) -> Option<String> {
    named(watching, path).map(|_| path.to_owned())
}

/// The live stream the first live-stream path covering `path` names in it.
fn named(watching: &Watching, path: &str) -> Option<String> {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    watching.stream(path, query)
}

/// The pass `member` holds for the live stream `path` of `open` names: the same one
/// every time, written to lapse when `open`'s passes were first issued to.
fn passed(ready: &Ready, member: &Member, open: &Open, path: &str) -> Option<String> {
    Some(
        ready
            .key
            .issued(&named(&ready.watching, path)?, &member.id, open.issued),
    )
}

/// Where each of `open`'s paths is served at the door, each carrying `member`'s pass for
/// the live stream it names, which lapses at `lapses`.
fn located(ready: &Ready, member: &Member, open: &Open, lapses: u64) -> TunedReport {
    let at = |path: &Option<String>| {
        let path = path.as_deref()?;
        let pass = passed(ready, member, open, path)?;
        joined(&ready.base, &door::passed(path, &pass))
    };
    TunedReport {
        channel: open.channel.clone(),
        member: member.name.clone(),
        stream: open.streams.first().cloned(),
        direct: at(&open.direct),
        transcoded: at(&open.transcoded),
        fingerprint: Some(ready.fingerprint.clone()),
        lapses: Some(lapses),
        unlocated: None,
        rehearsed: false,
    }
}

/// What opening or renewing `channel` for `member` would come to at the door `ready`
/// holds, with nothing opened and no pass issued.
fn rehearsed(channel: &str, member: &Member, ready: Ready) -> TunedReport {
    TunedReport {
        channel: channel.to_owned(),
        member: member.name.clone(),
        fingerprint: Some(ready.fingerprint),
        rehearsed: true,
        ..TunedReport::default()
    }
}

/// Nothing located for `channel`, and `why`.
fn unlocated(channel: &str, member: &Member, why: String) -> TunedReport {
    TunedReport {
        channel: channel.to_owned(),
        member: member.name.clone(),
        unlocated: Some(why),
        ..TunedReport::default()
    }
}

/// The refusal for a member with no live TV.
fn no_live_tv() -> Box<Problem> {
    refused(NO_LIVE_TV, "There is no live TV for this member")
}

/// Said where the pass service has no key to issue passes with.
const KEYLESS: &str = "The pass service has no key yet, so no pass can be issued for a live \
                       stream. It is made when the stack starts with a guard in front of the \
                       media server.";

/// Said where the media server's live-stream paths do not compile.
const UNDECLARED: &str = "The media server declared live-stream paths the core cannot hold a \
                          request to, so no pass is issued for a live stream.";

/// Said where no live-stream path covers a path the media server answered.
const UNCOVERED: &str = "The media server answered paths for the live stream that none of the \
                         live-stream paths it declared covers, so the guard would not ask about \
                         them and no pass is issued. The live stream was closed.";

/// Said where the pass service could not be told a renewal.
const UNRENEWED: &str = "The pass service could not be told the passes are held for longer, \
                         so they lapse when they were issued to. Check the project directory \
                         can be written.";

/// Said where there is nowhere to keep what closes a live stream.
const NOWHERE: &str = "There is no configuration directory to keep what closes a live stream \
                       in, so none is opened.";

#[cfg(test)]
mod tests;
