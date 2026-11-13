//! A title played in a member's browser, on a pass that holds a session on their own
//! account.
//!
//! A browser can neither pin the guarded door's certificate nor keep a session out of the
//! page's reach, so the core opens a session for the browser as it does for a paired
//! client's grant, seals its token inside an item pass for that title and that member,
//! and answers where the title plays from at the stack's front door, which presents a
//! certificate the browser trusts. The core keeps no copy of the token: a pass handed
//! back is opened and, where it is for the member and the title it names, the same pass
//! is held good for longer, so a player goes on fetching the address it began with.

use crate::app::{Asker, Ctx, PlayFrom};
use crate::error::codes::play::{
    NOT_A_DEVICE, NOT_ON_THEIR_SHELF, NOT_THEIR_PASS, SIGNS_NO_DEVICE_IN,
};
use crate::error::Problem;
use crate::model::PlayReport;
use crate::pass::{Key, Renewals, Session, LIFETIME, NONCE_BYTES};
use crate::ports::service::{Locations, Member};

use super::door::{self, told, PLAY_BYTES};
use super::{a_device, an_item, grants, member, not_an_item, refused, server, unanswered};

/// Where `id` plays from in `named`'s browser, on a pass made from what `from` names.
///
/// # Errors
///
/// A [`Problem`] where the id names nothing the server could hold, the device is not a
/// device id, the title is not on the member's shelf, the pass handed back is not this
/// member's for this title, the server signs no device in by code, or there is no
/// server or it does not answer.
pub(super) async fn played(
    ctx: &Ctx,
    named: &str,
    id: &str,
    from: PlayFrom,
    asker: Asker,
) -> Result<PlayReport, Box<Problem>> {
    if !an_item(id) {
        return Err(not_an_item());
    }
    if let PlayFrom::Device(device) = &from {
        if !a_device(device) {
            return Err(refused(NOT_A_DEVICE, "That is not a device id"));
        }
    }
    let server = server(ctx).await?;
    let member = member(server.identity.as_ref(), named).await?;
    let unlocated = |why| PlayReport {
        id: id.to_owned(),
        member: member.name.clone(),
        unlocated: Some(why),
        ..PlayReport::default()
    };
    let ready = match ready(ctx, &server, asker, id).await {
        Ok(ready) => ready,
        Err(why) => return Ok(unlocated(why)),
    };
    let rehearsal = || PlayReport {
        id: id.to_owned(),
        member: member.name.clone(),
        rehearsed: true,
        ..PlayReport::default()
    };
    let now = ctx.seconds();
    let lapses = now.saturating_add(LIFETIME);
    let pass = match from {
        PlayFrom::Pass(pass) => {
            reopened(&ready.key, &member, id, &pass)?;
            if ctx.dry_run {
                return Ok(rehearsal());
            }
            if !crate::app::project::passing::renewed(
                ctx,
                std::slice::from_ref(&pass),
                &member.id,
                lapses,
            ) {
                return Ok(unlocated(told(asker, UNRENEWED.to_owned())));
            }
            pass
        }
        PlayFrom::Device(device) => {
            server
                .serve
                .title(Some(&member.id), id)
                .await
                .map_err(|_| unanswered())?
                .ok_or_else(|| refused(NOT_ON_THEIR_SHELF, "That title is not on this shelf"))?;
            if ctx.dry_run {
                return Ok(rehearsal());
            }
            let session = Session {
                member: member.id.clone(),
                token: opened(&server, &member, &device).await?,
            };
            let Some(pass) = ready.key.seal(id, &session, ready.nonce, now) else {
                return Ok(unlocated(told(asker, UNSEALED.to_owned())));
            };
            pass
        }
    };
    grants::renewed(ctx, &member.id);
    let Some(playlist) = Locations::played(&ready.template, &ready.base, id, &play_of(&pass))
    else {
        return Ok(unlocated(told(asker, UNPLACED.to_owned())));
    };
    Ok(PlayReport {
        id: id.to_owned(),
        member: member.name,
        stream_from: Some(door::passed(&playlist, &pass)),
        lapses: Some(lapses),
        unlocated: None,
        rehearsed: false,
    })
}

/// The id of the play `pass` is for, which its location names where the template holds
/// `{play}`: drawn from the nonce the pass was sealed behind, so every new pass names a
/// fresh one and a renewal answers the same location.
fn play_of(pass: &str) -> String {
    let nonce = pass.split('.').next().unwrap_or_default();
    let digest = ring::digest::digest(&ring::digest::SHA256, nonce.as_bytes());
    crate::secret::render(digest.as_ref().get(..PLAY_BYTES).unwrap_or_default())
}

/// The session the pass a browser handed back seals, where it opens under the key for
/// `id` and is `member`'s. One that lapsed still opens, so a browser that paused past
/// its pass plays on.
fn reopened(key: &Key, member: &Member, id: &str, pass: &str) -> Result<Session, Box<Problem>> {
    key.open(pass, id, 0, &Renewals::none())
        .map(|opened| opened.session)
        .filter(|session| session.member == member.id)
        .ok_or_else(|| {
            refused(
                NOT_THEIR_PASS,
                "That pass is not this member's for this title",
            )
        })
}

/// A new session on `member`'s own account for the browser `device`, and its token.
async fn opened(
    server: &crate::app::targets::Media,
    member: &Member,
    device: &str,
) -> Result<String, Box<Problem>> {
    server
        .serve
        .signed_in(&member.id, device)
        .await
        .map_err(|_| unanswered())?
        .ok_or_else(|| {
            refused(
                SIGNS_NO_DEVICE_IN,
                "The media server signs no device in by code",
            )
        })
}

/// A nonce drawn for one pass, or nothing where no randomness could be drawn.
fn nonce(ctx: &Ctx) -> Option<[u8; NONCE_BYTES]> {
    ctx.seams
        .random
        .bytes(NONCE_BYTES)
        .and_then(|bytes| <[u8; NONCE_BYTES]>::try_from(bytes).ok())
}

/// Everything a browser's location is built from.
struct Ready {
    /// Where a browser plays an item from, as the media server declared it.
    template: String,
    /// The front door's address for a browser.
    base: String,
    /// The key passes are sealed with.
    key: Key,
    /// The nonce a new pass is sealed behind.
    nonce: [u8; NONCE_BYTES],
}

/// Everything `id`'s location in a browser is built from, or why nothing can be
/// located, in the words `asker` is told.
async fn ready(
    ctx: &Ctx,
    server: &crate::app::targets::Media,
    asker: Asker,
    id: &str,
) -> Result<Ready, String> {
    let template = door::declared(server.serve.as_ref())
        .await
        .map_err(|why| told(asker, why))?
        .locations
        .playlist
        .ok_or_else(|| told(asker, NO_PLAYLIST.to_owned()))?;
    let base = door::for_a_browser(ctx).map_err(|why| told(asker, why))?;
    Locations::played(
        &template,
        &base,
        id,
        &crate::secret::render(&[0; PLAY_BYTES]),
    )
    .ok_or_else(|| told(asker, UNPLACED.to_owned()))?;
    let key =
        crate::app::project::passing::key(ctx).ok_or_else(|| told(asker, KEYLESS.to_owned()))?;
    let nonce = nonce(ctx).ok_or_else(|| told(asker, UNSEALED.to_owned()))?;
    Ok(Ready {
        template,
        base,
        key,
        nonce,
    })
}

/// Said where the media server declares no playlist a browser plays an item from.
const NO_PLAYLIST: &str = "The media server declares no playlist a browser plays a title from, \
                           so nothing plays in a browser. A media server's plugin declares one \
                           from media.serve@1 minor 4.";

/// Said where the pass service has no key to seal passes with.
const KEYLESS: &str = "The pass service has no key yet, so no pass can be sealed for a \
                       browser. It is made when the stack starts with a guard in front of the \
                       media server.";

/// Said where no randomness could be drawn for a pass's nonce.
const UNSEALED: &str = "No randomness could be drawn for the pass, so none was sealed and \
                        nothing plays in a browser.";

/// Said where the pass service could not be told a renewal.
const UNRENEWED: &str = "The pass service could not be told the pass is held for longer, so \
                         it lapses when it was issued to. Check the project directory can be \
                         written.";

/// Said where the playlist does not stay on the front door's address once built.
const UNPLACED: &str = "The playlist the media server declared does not stay on the front \
                        door's address once the title is filled in, so nothing is located \
                        for a browser.";

#[cfg(test)]
mod tests;
