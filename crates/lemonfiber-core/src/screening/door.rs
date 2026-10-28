//! Where each title is served at the guarded front door, and the certificate it presents.
//!
//! Every location is built here from the household address the stack publishes, the
//! door's port and the item's own path, so a client is handed a whole address and never
//! puts one together. Where any part is missing, nothing is located and each title says
//! why: an address that was guessed is one somebody's phone goes and fails at.

use std::path::{Path, PathBuf};

use crate::app::Ctx;
use crate::certificate::{self, Kept, Unkept};
use crate::model::{Episode, Held, Located, PartWay, Pinned, Season, Title};
use crate::ports::service::{Declaring, Item, ItemDetail, ItemProgress, Locations, Medium};

/// The port the guarded front door serves encrypted on.
///
/// The bundled media server's own port for encrypted service, so a client that knows the
/// media server knows this one too.
pub const GUARDED: u16 = 8920;

/// Where the door's certificate and key are kept, beneath the stack directory.
pub const DOOR: &str = "config/door";

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

/// The address the door reaches the media server from, as the project's networks fix
/// it, or nothing where nothing guards the household's streams in front of a media
/// server.
///
/// Derived from the networks rather than written down here, so the address the media
/// server is told to trust is the one the door is given, on the day either moves.
#[must_use]
pub fn upstream(topology: &crate::topology::Topology) -> Option<String> {
    topology.guard_on_media().map(|address| address.to_string())
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
        /// Where the media server behind the door serves each item, as it declared it.
        locations: Locations,
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
pub fn ensured(
    confined: &dyn crate::ports::filesystem::Confined,
    project: &Path,
) -> Result<Kept, Unkept> {
    certificate::kept_or_made(confined, &directory(project))
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
        let _ = ensured(ctx.seams.confined.as_ref(), &project);
    }
}

/// The door as this machine stands now, in front of `server`.
pub(crate) async fn standing(ctx: &Ctx, server: &dyn Declaring) -> Door {
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let Some(project) = project else {
        return Door::Unknown(NO_STACK.to_owned());
    };
    let at = crate::app::invite::household_address(ctx, GUARDED).await;
    let (base, fingerprint) = match at_the_door(
        at.map(|address| address.url),
        certificate::kept(ctx.seams.confined.as_ref(), &directory(&project)),
    ) {
        Ok(door) => door,
        Err(why) => return Door::Unknown(why),
    };
    match served_at(server).await {
        Ok(locations) => Door::At {
            base,
            fingerprint,
            locations,
        },
        Err(why) => Door::Unknown(why),
    }
}

/// The door's address and its certificate's fingerprint, from its address and what is
/// kept for its certificate, or why there is neither.
fn at_the_door(
    url: Option<String>,
    kept: Result<Option<Kept>, Unkept>,
) -> Result<(String, String), String> {
    let fingerprint = match kept {
        Ok(Some(kept)) => kept.fingerprint,
        Ok(None) => return Err(NOT_MADE.to_owned()),
        Err(why) => return Err(format!("{UNREADABLE} {why}")),
    };
    url.map(|url| (url.replacen("http://", "https://", 1), fingerprint))
        .ok_or_else(|| NO_ADDRESS.to_owned())
}

/// Where `server` serves each item, from its declaration once that is checked.
async fn served_at(server: &dyn Declaring) -> Result<Locations, String> {
    let declared = server.declared().await.map_err(|_| UNDECLARED.to_owned())?;
    declared
        .checked()
        .map(|declared| declared.locations)
        .map_err(|why| format!("{REFUSED_DECLARATION} {why}"))
}

/// Said where the media server would not say where it serves each item.
const UNDECLARED: &str = "The media server would not say where it serves each title, so \
                          nothing can be located at the front door.";

/// Said before the reason the media server's declaration was refused.
const REFUSED_DECLARATION: &str = "The media server declared where it serves each title in a \
                                   way the front door cannot hold it to, so nothing is \
                                   located there:";

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

/// Why an item the media server named by something other than an item id is not located.
const NOT_AN_ITEM: &str = "the media server named this item by something that is not an item id";

/// The same item, located at the door: its pictures where it has them, and where it
/// streams from where it plays.
#[must_use]
pub(crate) fn located(item: Item, door: &Door) -> Held {
    let at = match door {
        _ if !super::an_item(&item.id) => Located {
            unlocated: Some(NOT_AN_ITEM.to_owned()),
            ..Located::default()
        },
        Door::Unknown(why) => Located {
            unlocated: Some(why.clone()),
            ..Located::default()
        },
        Door::At {
            base,
            fingerprint,
            locations: templates,
        } => {
            let at = |template: &str| Locations::located(template, base, &item.id);
            Located {
                poster: item.holds.poster.then(|| at(&templates.poster)).flatten(),
                backdrop: item
                    .holds
                    .backdrop
                    .then(|| at(&templates.backdrop))
                    .flatten(),
                stream_from: streams(&item).then(|| at(&templates.stream)).flatten(),
                door: Some(Pinned {
                    fingerprint: fingerprint.clone(),
                }),
                unlocated: None,
            }
        }
    };
    Held {
        id: item.id,
        title: item.title,
        year: item.year,
        medium: item.medium,
        at,
    }
}

/// A title and every episode in it, located at the door.
pub(crate) fn placed(detail: ItemDetail, door: &Door) -> Title {
    Title {
        held: located(detail.item, door),
        overview: detail.overview,
        minutes: detail.minutes,
        genres: detail.genres,
        certificate: detail.certificate,
        released: detail.released,
        seasons: detail
            .seasons
            .into_iter()
            .map(|season| Season {
                id: season.id,
                name: season.name,
                number: season.number,
                episodes: season
                    .episodes
                    .into_iter()
                    .map(|episode| Episode {
                        held: located(episode.item, door),
                        number: episode.number,
                        overview: episode.overview,
                        minutes: episode.minutes,
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// How far a member got through one item, located at the door.
pub(crate) fn progressed(progress: ItemProgress, door: &Door) -> PartWay {
    PartWay {
        held: located(progress.item, door),
        position: progress.position,
        length: progress.length,
    }
}

/// Whether an item is one that streams: a film or an episode, rather than a series that
/// holds episodes or something this product does not play.
fn streams(item: &Item) -> bool {
    item.holds.plays && matches!(item.medium, Medium::Film | Medium::Episode)
}

#[cfg(test)]
mod tests;
