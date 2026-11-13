//! Where each title is served at the guarded front door, and the certificate it presents.
//!
//! Every location is built here from the household address the stack publishes, the
//! door's port and the item's own path, so a client is handed a whole address and never
//! puts one together. Where any part is missing, nothing is located and each title says
//! why: an address that was guessed is one somebody's phone goes and fails at.

use std::path::{Path, PathBuf};

use lemonfiber_contract::capabilities::proxy::front;
use lemonfiber_contract::capabilities::stream::guard;

use crate::app::{Asker, Ctx};
use crate::certificate::{self, Kept, Unkept};
use crate::model::{Episode, Held, Located, PartWay, Pinned, Season, Title};
use crate::ports::random::Random;
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
        /// Who asked, which decides the words an item not located says why in.
        asker: Asker,
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

/// The door as this machine stands now, in front of `server`, saying why nothing can
/// be located there in the words `asker` is told.
pub(crate) async fn standing(ctx: &Ctx, server: &dyn Declaring, asker: Asker) -> Door {
    match seen(ctx, server, asker).await {
        Door::Unknown(why) => Door::Unknown(told(asker, why)),
        door => door,
    }
}

/// The door as this machine stands now, in front of `server`.
async fn seen(ctx: &Ctx, server: &dyn Declaring, asker: Asker) -> Door {
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
            asker,
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
    declared(server).await.map(|declared| declared.locations)
}

/// What `server` declared, once that is checked, or why it is not held to it.
pub(crate) async fn declared(
    server: &dyn Declaring,
) -> Result<crate::ports::service::Declared, String> {
    let declared = server.declared().await.map_err(|_| UNDECLARED.to_owned())?;
    declared
        .checked()
        .map_err(|why| format!("{REFUSED_DECLARATION} {why}"))
}

/// `why` nothing is located, in the words `asker` is told: what to fix for the operator,
/// and for a member only that it does not play here yet, which nothing they can do
/// changes.
#[must_use]
pub(crate) fn told(asker: Asker, why: String) -> String {
    match asker {
        Asker::Operator => why,
        Asker::Member => UNPLAYABLE.to_owned(),
    }
}

/// A fresh id for one play of an item, which a stream or playlist location names where
/// its template holds `{play}`: thirty-two lower-case hex digits, or nothing where no
/// randomness could be drawn.
#[must_use]
pub(crate) fn drawn_play(random: &dyn Random) -> Option<String> {
    random
        .bytes(PLAY_BYTES)
        .map(|bytes| crate::secret::render(&bytes))
}

/// How many bytes a play's id is drawn from.
pub(crate) const PLAY_BYTES: usize = 16;

/// `at`, a path or a whole address, carrying `pass` in the one parameter a pass travels
/// in. A pass is written in characters a query carries as they are.
#[must_use]
pub(crate) fn passed(at: &str, pass: &str) -> String {
    let joiner = if at.contains('?') { '&' } else { '?' };
    format!("{at}{joiner}{}={pass}", lemonfiber_sidecar::pass::PARAMETER)
}

/// What a member is told where nothing can be located for them.
const UNPLAYABLE: &str = "This cannot be played here yet. Whoever looks after the household \
                          can set it up.";

/// The address the stack's front door serves the guard at for a browser, with a
/// certificate a browser trusts: the label the guard's route is published under, in front
/// of the domain the household's proxy serves; or why there is none.
pub(crate) async fn for_a_browser(ctx: &Ctx) -> Result<String, String> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|_| NO_BROWSER_ROUTE.to_owned())?;
    let register = crate::app::plugins::read(ctx).map_err(|_| NO_BROWSER_ROUTE.to_owned())?;
    let fillers = crate::app::targets::fillers_here(ctx, &manifest);
    if fillers.filling(front::CAPABILITY).is_none() {
        return Err(NO_PROXY.to_owned());
    }
    let label = fillers
        .filling(guard::CAPABILITY)
        .and_then(|(guarding, _)| {
            register
                .installed()
                .iter()
                .flat_map(|plugin| &plugin.services)
                .find(|placed| placed.service == guarding.id)
        })
        .and_then(|placed| placed.reached.as_ref()?.hostname().map(str::to_owned))
        .ok_or_else(|| NO_BROWSER_ROUTE.to_owned())?;
    crate::door::proxied(&label, ctx.settings.household_domain.as_deref())
        .map(|address| address.url)
        .ok_or_else(|| NO_DOMAIN.to_owned())
}

/// Said where no proxy fronts the household's services, so no browser reaches the door.
const NO_PROXY: &str = "No proxy fronts the household's services, so a browser has no address \
                        with a certificate it trusts to play from. Install the household's \
                        proxy, and every title plays in a browser.";

/// Said where the guard publishes no route through the household's proxy.
const NO_BROWSER_ROUTE: &str = "The guard in front of the media server publishes no route \
                                through the household's proxy, so a browser has nowhere to \
                                play from. Install the door plugin, which publishes one.";

/// Said where no domain is configured for the household's proxy to serve under.
const NO_DOMAIN: &str = "No domain is configured for the household's proxy, so the route a \
                         browser plays from has no address a certificate is issued for. Set \
                         DOMAIN to a domain whose addresses point at this machine, and every \
                         title plays in a browser.";

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
pub(crate) fn located(item: Item, door: &Door, random: &dyn Random) -> Held {
    let at = match door {
        Door::Unknown(why) => Located {
            unlocated: Some(why.clone()),
            ..Located::default()
        },
        Door::At { asker, .. } if !super::an_item(&item.id) => Located {
            unlocated: Some(told(*asker, NOT_AN_ITEM.to_owned())),
            ..Located::default()
        },
        Door::At {
            base,
            fingerprint,
            locations: templates,
            ..
        } => {
            let at = |template: &str| Locations::located(template, base, &item.id);
            let stream_from = streams(&item)
                .then(|| {
                    let play = drawn_play(random)?;
                    Locations::played(&templates.stream, base, &item.id, &play)
                })
                .flatten();
            Located {
                poster: item.holds.poster.then(|| at(&templates.poster)).flatten(),
                backdrop: item
                    .holds
                    .backdrop
                    .then(|| at(&templates.backdrop))
                    .flatten(),
                stream_from,
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
pub(crate) fn placed(detail: ItemDetail, door: &Door, random: &dyn Random) -> Title {
    Title {
        held: located(detail.item, door, random),
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
                        held: located(episode.item, door, random),
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
pub(crate) fn progressed(progress: ItemProgress, door: &Door, random: &dyn Random) -> PartWay {
    PartWay {
        held: located(progress.item, door, random),
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
