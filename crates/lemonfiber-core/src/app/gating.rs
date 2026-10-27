//! The request gate, as the stack declares it and as its files are placed.
//!
//! The gate stands between the request service and the services it fulfils requests
//! through, holding their credentials so the request service holds only tokens
//! lemonfiber minted for the gate. Its files live in its configuration directory, in
//! the shapes [`lemonfiber_sidecar::gate`] defines.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_contract::capabilities::request::gate;
use lemonfiber_sidecar::gate::{File, Kind};

use crate::app::targets::{spoken, Spoken};
use crate::app::Ctx;
use crate::origin::Origin;
use crate::ports::media;
use crate::ports::service::Gating;
use crate::wiring::Fillers;

mod configured;

/// The request gate's id in the stack manifest.
pub(crate) const SERVICE: &str = "request-gate";

/// The request gate, where the stack runs one.
pub(crate) fn service(
    services: &[lemonfiber_manifest::Service],
) -> Option<&lemonfiber_manifest::Service> {
    services.iter().find(|service| service.id == SERVICE)
}

/// The request gate as the core asks it: over `request.gate` where its filler speaks the
/// contract, otherwise the stack's own gate told through its files under `project`. Nothing
/// where nothing fills the capability, its filler is not one the stack's credentials may be
/// handed to, or it speaks the contract and cannot be asked over it.
pub(crate) async fn asked(ctx: &Ctx, fillers: &Fillers, project: &Path) -> Option<Arc<dyn Gating>> {
    let (filler, _) = fillers.filling(gate::CAPABILITY).filter(|(filler, _)| {
        crate::wiring::crosses(crate::wiring::Holder::Stack, filler.holder())
    })?;
    match spoken(ctx, filler, gate::CAPABILITY, gate::MAJOR).await {
        Spoken::Over(adapter) => Some(Arc::new(gate::Adapter(adapter))),
        Spoken::Unanswered => None,
        Spoken::Not => (filler.origin == Origin::Bundled).then(|| {
            Arc::new(configured::Configured::at(
                project,
                ctx.seams.filesystem.clone(),
            )) as Arc<dyn Gating>
        }),
    }
}

/// Whether a credential `owner` holds may be handed to the service filling `request.gate`.
pub(crate) fn may_hold(fillers: &Fillers, owner: crate::wiring::Holder<'_>) -> bool {
    fillers
        .filling(gate::CAPABILITY)
        .is_some_and(|(filler, _)| crate::wiring::crosses(owner, filler.holder()))
}

/// Where one of the gate's files lives: its configuration directory, under the project
/// root the stack's config volumes are mounted from.
pub(crate) fn path(project: &Path, file: File) -> PathBuf {
    project.join("config").join(SERVICE).join(file.name())
}

/// The kind of video the curator a route reaches fetches, or `None` for the media
/// server's route.
pub(crate) const fn fetching(route: Kind) -> Option<media::Kind> {
    match route {
        Kind::Sonarr => Some(media::Kind::Tv),
        Kind::Radarr => Some(media::Kind::Movies),
        Kind::Jellyfin => None,
    }
}

/// The kind of video the curator a route reaching `target` fetches, or `None` for the media
/// server's route or a curator of anything else.
pub(crate) fn fetched(target: &crate::ports::service::Target) -> Option<media::Kind> {
    match target {
        crate::ports::service::Target::Curator { media_type } => {
            media::Kind::for_media_type(media_type)
        }
        crate::ports::service::Target::MediaServer => None,
    }
}

/// The route kind for a curator fetching `kind`.
pub(crate) const fn route_for(kind: media::Kind) -> Kind {
    match kind {
        media::Kind::Tv => Kind::Sonarr,
        media::Kind::Movies => Kind::Radarr,
    }
}

#[cfg(test)]
mod tests;
