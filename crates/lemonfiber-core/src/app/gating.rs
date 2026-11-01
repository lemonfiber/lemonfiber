//! The request gate, as the stack declares it and as the core asks it.
//!
//! The gate stands between the request service and the services it fulfils requests
//! through, holding their credentials so the request service holds only tokens
//! lemonfiber minted for the gate. The core asks it over `request.gate` and no other way.

use std::sync::Arc;

use crate::app::targets::{spoken, Spoken};
use crate::app::Ctx;
use crate::ports::media;
use crate::ports::service::Gating;
use crate::wiring::Fillers;
use lemonfiber_contract::capabilities::request::gate;

/// The port the request gate answers its routes on, inside the stack's network.
pub(crate) const PORT: u16 = 5057;

/// The request gate's id in the stack manifest.
pub(crate) const SERVICE: &str = "request-gate";

/// The request gate, where the stack runs one.
pub(crate) fn service(
    services: &[lemonfiber_manifest::Service],
) -> Option<&lemonfiber_manifest::Service> {
    services.iter().find(|service| service.id == SERVICE)
}

/// The request gate as the core asks it: over `request.gate`, where its filler speaks the
/// contract. Nothing where nothing fills the capability, its filler is not one the stack's
/// credentials may be handed to, it speaks nothing, or it speaks the contract and cannot
/// be asked over it.
pub(crate) async fn asked(ctx: &Ctx, fillers: &Fillers) -> Option<Arc<dyn Gating>> {
    let (filler, _) = fillers.filling(gate::CAPABILITY).filter(|(filler, _)| {
        crate::wiring::crosses(crate::wiring::Holder::Stack, filler.holder())
    })?;
    match spoken(ctx, filler, gate::CAPABILITY, gate::MAJOR).await {
        Spoken::Over(adapter) => Some(Arc::new(gate::Adapter(adapter))),
        Spoken::Unanswered | Spoken::Not => None,
    }
}

/// Whether a credential `owner` holds may be handed to the service filling `request.gate`.
pub(crate) fn may_hold(fillers: &Fillers, owner: crate::wiring::Holder<'_>) -> bool {
    fillers
        .filling(gate::CAPABILITY)
        .is_some_and(|(filler, _)| crate::wiring::crosses(owner, filler.holder()))
}

/// The kind of video the curator a route reaching `target` fetches, or `None` for the media
/// server's route or a curator of anything else.
pub(crate) fn fetched(target: &crate::ports::service::Target) -> Option<media::Kind> {
    match target {
        crate::ports::service::Target::Curator { media_type } => {
            media::Kind::for_media_type(media_type)
        }
        crate::ports::service::Target::MediaServer { .. } => None,
    }
}

#[cfg(test)]
mod tests;
