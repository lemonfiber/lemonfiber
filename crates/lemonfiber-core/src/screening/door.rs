//! Where each title is served at the guarded front door, and the certificate it presents.
//!
//! Every location is built here from the household address the stack publishes, the
//! door's port and the item's own path, so a client is handed a whole address and never
//! puts one together. Where any part is missing, nothing is located and each title says
//! why: an address that was guessed is one somebody's phone goes and fails at.

use std::path::{Path, PathBuf};

use crate::app::Ctx;
use crate::certificate::{self, Kept, Unkept};
use crate::ports::service::{Held, Located, Medium, Pinned};

/// The port the guarded front door serves encrypted on.
///
/// Jellyfin's own port for encrypted service, so a client that knows the media server
/// knows this one too.
pub const GUARDED: u16 = 8920;

/// Where the door's certificate and key are kept, beneath the stack directory.
pub const DOOR: &str = "config/door";

/// The query a stream is asked for with: the item's own source, and the codecs every
/// supported player decodes, in segments both platforms play.
const STREAMED: &str = "VideoCodec=h264,hevc&AudioCodec=aac,mp3,ac3,eac3&SegmentContainer=mp4";

/// The service the stack runs as the guarded front door, by its id.
pub const SERVICE: &str = "door";

/// The port the household reaches the media server at: the door's, where the stack
/// runs one in front of it, or the media server's own where it does not.
///
/// The media server publishes on this machine alone behind a door, so its own port is
/// one nobody in the house can reach, and an address built on it is one somebody's
/// television goes and fails at.
#[must_use]
pub fn household_port(manifest: &lemonfiber_manifest::Manifest, own: u16) -> u16 {
    manifest
        .services
        .iter()
        .find(|service| service.id == SERVICE)
        .and_then(|service| service.port)
        .unwrap_or(own)
}

/// The stack's compose file that declares the door, beneath the stack directory.
const COMPOSE: &str = "compose/media.yml";

/// The network the door reaches the media server on.
const UPSTREAM: &str = "door-upstream";

/// The address the door reaches the media server from, as the stack's compose file fixes
/// it, or nothing where the stack declares no door or fixes no address for it.
///
/// Read from the file rather than written down here, so the address the media server is
/// told to trust is the one the door is given, on the day either moves.
#[must_use]
pub fn upstream(project: &Path) -> Option<String> {
    let text = std::fs::read_to_string(project.join(COMPOSE)).ok()?;
    crate::stack::declared::fixed_address(&text, SERVICE, UPSTREAM)
}

/// The door as a location is built against: its address and the certificate it
/// presents, or why there is no location to build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Door {
    /// Where the door is, and the fingerprint a client pins it by.
    At {
        /// The door's address, scheme and port included, with no path.
        base: String,
        /// SHA-256 over the door's certificate, lower-case hex.
        fingerprint: String,
    },
    /// Why nothing can be located at the door.
    Unknown(String),
}

/// The directory the door's certificate is kept in, beneath `project`.
#[must_use]
pub fn directory(project: &Path) -> PathBuf {
    project.join(DOOR)
}

/// The door's certificate, made first where none has been.
///
/// Made before the stack starts, because the door will not serve without one.
///
/// # Errors
///
/// [`Unkept`] where what is kept cannot be read, or a new one cannot be made.
pub fn ensured(project: &Path) -> Result<Kept, Unkept> {
    certificate::kept_or_made(&directory(project))
}

/// Make the door's certificate before the stack starts, where the stack runs a door.
///
/// Beside the other secrets a service needs before it has ever run: the door will not
/// serve without a certificate, and nothing renews one once it is made, so one made
/// here is the one every phone pins.
pub(crate) fn made_before_start(ctx: &Ctx, manifest: &lemonfiber_manifest::Manifest) {
    let runs_a_door = manifest
        .services
        .iter()
        .any(|service| service.id == SERVICE);
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    if let (true, Some(project)) = (runs_a_door, project) {
        let _ = ensured(&project);
    }
}

/// The door as this machine stands now.
pub(crate) async fn standing(ctx: &Ctx) -> Door {
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let Some(project) = project else {
        return Door::Unknown(NO_STACK.to_owned());
    };
    let at = crate::app::invite::household_address(ctx, GUARDED).await;
    at_the_door(
        at.map(|address| address.url),
        certificate::kept(&directory(&project)),
    )
}

/// The door from its address and what is kept for its certificate.
fn at_the_door(url: Option<String>, kept: Result<Option<Kept>, Unkept>) -> Door {
    let fingerprint = match kept {
        Ok(Some(kept)) => kept.fingerprint,
        Ok(None) => return Door::Unknown(NOT_MADE.to_owned()),
        Err(why) => return Door::Unknown(format!("{UNREADABLE} {why}")),
    };
    match url {
        Some(url) => Door::At {
            base: url.replacen("http://", "https://", 1),
            fingerprint,
        },
        None => Door::Unknown(NO_ADDRESS.to_owned()),
    }
}

/// Said where there is no stack directory, so nothing is kept for the door.
const NO_STACK: &str = "There is no stack directory, so the front door has no certificate to \
                        present and nothing is served through it.";

/// Said where the door's certificate has not been made yet.
const NOT_MADE: &str = "The front door has no certificate yet. It is made when the stack \
                        starts, and nothing is served through the door before then.";

/// Said before the reason the door's certificate could not be read.
const UNREADABLE: &str = "The front door's certificate could not be read, so nothing can be \
                          pinned to it:";

/// Said where the household address is not known.
const NO_ADDRESS: &str = "This machine's household address is not known, so nothing can be \
                          located at its front door. Record the address the household reaches \
                          it at, and every title says where it is served.";

/// The same item, located at the door: its pictures where it has them, and where it
/// streams from where it plays.
#[must_use]
pub(crate) fn located(mut held: Held, door: &Door) -> Held {
    held.at = match door {
        Door::Unknown(why) => Located {
            unlocated: Some(why.clone()),
            ..Located::default()
        },
        Door::At { base, fingerprint } => {
            let id = &held.id;
            Located {
                poster: held
                    .holds
                    .poster
                    .then(|| format!("{base}/Items/{id}/Images/Primary")),
                backdrop: held
                    .holds
                    .backdrop
                    .then(|| format!("{base}/Items/{id}/Images/Backdrop")),
                stream_from: streams(&held).then(|| {
                    format!("{base}/Videos/{id}/master.m3u8?MediaSourceId={id}&{STREAMED}")
                }),
                door: Some(Pinned {
                    fingerprint: fingerprint.clone(),
                }),
                unlocated: None,
            }
        }
    };
    held
}

/// Whether an item is one that streams: a film or an episode, rather than a series that
/// holds episodes or something this product does not play.
fn streams(held: &Held) -> bool {
    held.holds.plays && matches!(held.medium, Medium::Film | Medium::Episode)
}

#[cfg(test)]
mod tests;
