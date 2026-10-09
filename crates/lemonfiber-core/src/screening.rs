//! What a member plays: the grant their client plays on, what a title is, how far through
//! it they got, and the progress a player reports back.
//!
//! **Every answer is the media server's, about the member's own account.** It applies
//! their library access and age limit before it answers, so nothing here filters, and a
//! title outside their limits is answered as absent — the same answer a title the
//! household does not hold gets, so the one cannot be told from the other.
//!
//! **Every location is the core's.** Each title is located at the guarded front door
//! with the fingerprint the door presents, so a client is handed a whole address it can
//! pin and never builds one.

pub mod door;
pub mod grants;

use crate::app::targets::{media, Media};
use crate::app::{Ctx, Outcome, Viewing, Whom};
use crate::error::codes::play::{
    NOBODY_NAMED, NOTHING_TO_PLAY_FROM, NOT_AN_ITEM, NOT_A_DEVICE, NOT_IN_THE_HOUSEHOLD,
    NOT_ON_THEIR_SHELF, NO_SUCH_PICTURE, SERVER_SILENT, SIGNS_NO_DEVICE_IN,
};
use crate::error::{Code, Diagnose as _, Problem, Remedy, State};
use crate::model::{GrantReport, PartWayReport, TitleReport, WatchedReport};
use crate::ports::service::{Household, HowFar, Image, Member, Picture, PICTURE_MOST};

use door::{placed, progressed};

/// Every refusal a member's viewing answers with, which a client names by its code.
pub const REFUSALS: [Code; 9] = [
    NOT_AN_ITEM,
    NOT_ON_THEIR_SHELF,
    NOT_A_DEVICE,
    NOTHING_TO_PLAY_FROM,
    SERVER_SILENT,
    NOBODY_NAMED,
    NOT_IN_THE_HOUSEHOLD,
    SIGNS_NO_DEVICE_IN,
    NO_SUCH_PICTURE,
];

/// How many titles a member's part-way list answers with.
pub const A_FEW: u32 = 24;

/// The media types a picture is passed on as: raster images, which carry nothing a
/// browser runs.
pub const RASTER: [&str; 5] = [
    "image/jpeg",
    "image/png",
    "image/webp",
    "image/gif",
    "image/avif",
];

/// A picture as the core passes it on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pictured {
    /// Its media type, one of [`RASTER`].
    pub media_type: &'static str,
    /// Its bytes, at most [`PICTURE_MOST`] of them.
    pub bytes: Vec<u8>,
}

/// The picture as the core passes it on, where it is a raster image of at most
/// [`PICTURE_MOST`] bytes.
fn passed_on(image: Image) -> Option<Pictured> {
    let declared = image
        .media_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim();
    let media_type = RASTER
        .into_iter()
        .find(|raster| raster.eq_ignore_ascii_case(declared))?;
    (image.bytes.len() <= PICTURE_MOST).then_some(Pictured {
        media_type,
        bytes: image.bytes,
    })
}

/// What one of a member's viewing requests comes to.
///
/// # Errors
///
/// The [`Problem`] the request is refused with.
pub(crate) async fn viewed(ctx: &Ctx, viewing: Viewing) -> Result<Outcome, Box<Problem>> {
    match viewing {
        Viewing::Title { member, id } => title(ctx, &member, &id).await.map(Outcome::Title),
        Viewing::PartWay { member, most } => {
            part_way(ctx, &member, most).await.map(Outcome::PartWay)
        }
        Viewing::Grant { member, device } => {
            granted(ctx, &member, &device).await.map(Outcome::Granted)
        }
        Viewing::Watched {
            member,
            id,
            how_far,
        } => watched(ctx, &member, &id, how_far)
            .await
            .map(Outcome::Watched),
    }
}

/// What one title is, as the member it was asked for may see it.
///
/// # Errors
///
/// A [`Problem`] where the id names nothing the server could hold, where the title is
/// not on the member's shelf, and where there is no server or it does not answer.
async fn title(ctx: &Ctx, whose: &Whom, id: &str) -> Result<TitleReport, Box<Problem>> {
    if !an_item(id) {
        return Err(refused(
            NOT_AN_ITEM,
            "That is not a title the household could hold",
        ));
    }
    let server = server(ctx).await?;
    let member = match whose {
        Whom::Named(named) => Some(member(server.identity.as_ref(), named).await?),
        Whom::Defaults => None,
    };
    let title = server
        .serve
        .title(member.as_ref().map(|member| member.id.as_str()), id)
        .await
        .map_err(|_| unanswered())?
        .ok_or_else(|| refused(NOT_ON_THEIR_SHELF, "That title is not on this shelf"))?;
    let door = door::standing(ctx).await;
    let (member, id) = member.map_or_else(Default::default, |member| (member.name, member.id));
    Ok(TitleReport {
        member,
        id,
        title: Some(placed(title, &door)),
        rehearsed: false,
    })
}

/// One of a title's pictures, as the member it was asked for may see it.
///
/// # Errors
///
/// A [`Problem`] where the id names nothing the server could hold, where the title is
/// not on the member's shelf or has no such picture, where what the server answered is
/// not a raster image of at most [`PICTURE_MOST`] bytes, and where there is no server or
/// it does not answer.
pub async fn picture(
    ctx: &Ctx,
    whose: &Whom,
    id: &str,
    which: Picture,
) -> Result<Pictured, Box<Problem>> {
    if !an_item(id) {
        return Err(refused(
            NOT_AN_ITEM,
            "That is not a title the household could hold",
        ));
    }
    let server = server(ctx).await?;
    let member = match whose {
        Whom::Named(named) => Some(member(server.identity.as_ref(), named).await?),
        Whom::Defaults => None,
    };
    server
        .serve
        .picture(member.as_ref().map(|member| member.id.as_str()), id, which)
        .await
        .map_err(|_| unanswered())?
        .and_then(passed_on)
        .ok_or_else(|| refused(NO_SUCH_PICTURE, "That title has no such picture"))
}

/// What one member was part-way through, most recent first.
///
/// # Errors
///
/// A [`Problem`] where nobody is named, where there is no server, and where the member
/// is not in the household. A server that will not say is answered as unread.
async fn part_way(ctx: &Ctx, whose: &Whom, most: u32) -> Result<PartWayReport, Box<Problem>> {
    let Whom::Named(named) = whose else {
        return Err(nobody());
    };
    let server = server(ctx).await?;
    let member = member(server.identity.as_ref(), named).await?;
    let Ok(part_way) = server.serve.part_way(&member.id, most).await else {
        return Ok(PartWayReport {
            member: member.name,
            id: member.id,
            findings: vec![UNREAD.to_owned()],
            ..PartWayReport::default()
        });
    };
    let door = door::standing(ctx).await;
    Ok(PartWayReport {
        member: member.name,
        id: member.id,
        part_way: part_way
            .into_iter()
            .map(|one| progressed(one, &door))
            .collect(),
        available: true,
        findings: Vec::new(),
        rehearsed: false,
    })
}

/// Open a session on `member`'s own account for one of their devices, and answer the
/// token it plays with.
///
/// The token is answered once and nothing keeps it: the core records only the day the
/// member's client last spoke, which is what bounds the grant.
///
/// # Errors
///
/// A [`Problem`] where the device is not named by a device id, there is no server, the
/// member is not in the household, the server signs no device in by code, or it does
/// not answer.
async fn granted(ctx: &Ctx, named: &str, device: &str) -> Result<GrantReport, Box<Problem>> {
    if !a_device(device) {
        return Err(refused(NOT_A_DEVICE, "That is not a device id"));
    }
    let server = server(ctx).await?;
    let member = member(server.identity.as_ref(), named).await?;
    let lasts_until = grants::until(ctx.today())
        .map(grants::written)
        .unwrap_or_default();
    if ctx.dry_run {
        return Ok(GrantReport {
            member: member.name,
            lasts_until,
            rehearsed: true,
            ..GrantReport::default()
        });
    }
    let token = server
        .serve
        .signed_in(&member.id, device)
        .await
        .map_err(|_| unanswered())?
        .ok_or_else(|| {
            refused(
                SIGNS_NO_DEVICE_IN,
                "The media server signs no device in by code",
            )
        })?;
    grants::renewed(ctx, &member.id);
    Ok(GrantReport {
        member: member.name,
        granted: true,
        token: Some(token),
        lasts_until,
        rehearsed: false,
    })
}

/// Record how far through one title `member` is, as their own progress.
///
/// # Errors
///
/// A [`Problem`] where the id names nothing the server could hold, there is no server,
/// the member is not in the household, or the server does not answer.
async fn watched(
    ctx: &Ctx,
    named: &str,
    id: &str,
    how_far: HowFar,
) -> Result<WatchedReport, Box<Problem>> {
    if !an_item(id) {
        return Err(refused(
            NOT_AN_ITEM,
            "That is not a title the household could hold",
        ));
    }
    let server = server(ctx).await?;
    let member = member(server.identity.as_ref(), named).await?;
    if !ctx.dry_run {
        server
            .serve
            .progressed(&member.id, id, &how_far)
            .await
            .map_err(|_| unanswered())?;
    }
    Ok(WatchedReport {
        id: id.to_owned(),
        position: how_far.position,
        ended: how_far.ended,
        rehearsed: ctx.dry_run,
    })
}

/// Renew the grant of a member whose client just spoke to the core.
pub fn spoke(ctx: &Ctx, member: &str) {
    if !ctx.dry_run {
        grants::renewed(ctx, member);
    }
}

/// Sign out the devices that played on grants that ran out.
///
/// Quietly nothing where there is no server to ask: the next run that can reach one
/// does it.
pub(crate) async fn lapsed(ctx: &Ctx) {
    if ctx.dry_run {
        return;
    }
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return;
    };
    if let Some(server) = media(ctx, &manifest).await {
        grants::ended(ctx, &server).await;
    }
}

/// Whether `id` is shaped like an item the media server files: thirty-two hex digits,
/// with or without the four dashes.
///
/// Held to that before it goes anywhere near a path, so a caller cannot reach another
/// endpoint by naming one.
#[must_use]
pub fn an_item(id: &str) -> bool {
    let digits: Vec<char> = id.chars().filter(|letter| *letter != '-').collect();
    let dashed = id.len() == 36
        && id
            .char_indices()
            .all(|(at, letter)| (letter == '-') == matches!(at, 8 | 13 | 18 | 23));
    (id.len() == 32 || dashed) && digits.len() == 32 && digits.iter().all(char::is_ascii_hexdigit)
}

/// Whether `device` is shaped like the id a player keeps for the device it plays on:
/// eight to sixty-four letters, digits and dashes.
///
/// Held to that before it goes anywhere near the media server, where it is written
/// between quotes it must not be able to close.
#[must_use]
pub fn a_device(device: &str) -> bool {
    (DEVICE_SHORTEST..=DEVICE_LONGEST).contains(&device.len())
        && device
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || letter == '-')
}

/// The fewest characters a device id has.
const DEVICE_SHORTEST: usize = 8;

/// The most characters a device id has.
const DEVICE_LONGEST: usize = 64;

/// The stack's media server, signed in as its administrator.
async fn server(ctx: &Ctx) -> Result<Media, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    media(ctx, &manifest).await.ok_or_else(|| {
        refused(
            NOTHING_TO_PLAY_FROM,
            "There is no media server to play from",
        )
    })
}

/// The member a name or an id means.
async fn member(server: &dyn Household, named: &str) -> Result<Member, Box<Problem>> {
    if named.trim().is_empty() {
        return Err(nobody());
    }
    let accounts = server.household().await.map_err(|_| unanswered())?;
    crate::app::held::account(&accounts, named)
        .cloned()
        .ok_or_else(|| {
            refused(
                NOT_IN_THE_HOUSEHOLD,
                "Nobody in this household is known by that",
            )
        })
}

/// Said where what was part-way through could not be read.
const UNREAD: &str = "The media server would not say what was part-way through, so it is \
                      reported as unread rather than as nothing.";

/// A refusal under `code`, with the registry's meaning and remedy.
fn refused(code: crate::error::Code, summary: &str) -> Box<Problem> {
    let declared = code.declaration();
    Box::new(
        Problem::new(
            code,
            summary,
            declared.map_or("", |declared| declared.meaning()),
            Remedy::new(declared.map_or("", |declared| declared.remedy())),
        )
        .in_state(State::Guided),
    )
}

/// The refusal for nobody named.
fn nobody() -> Box<Problem> {
    refused(NOBODY_NAMED, "Nobody was named")
}

/// The refusal for a server that did not answer.
fn unanswered() -> Box<Problem> {
    refused(SERVER_SILENT, "The media server did not answer")
}

#[cfg(test)]
mod tests;
