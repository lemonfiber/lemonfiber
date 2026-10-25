//! The egress guard and every torrent client inside it, each over its contract.

use lemonfiber_contract::capabilities::download::torrent;
use lemonfiber_contract::capabilities::network::egress;

use crate::app::targets::{declared_downloads, spoken, DownloadTarget, Spoken};
use crate::app::Ctx;
use crate::doctor::vpn::Gluetun;
use crate::origin::Origin;
use crate::wiring::{Filler, Fillers};

/// The filler of `network.egress-guard`: over its contract where it speaks one, the
/// bundled Gluetun where it is that or a first-party stand-in for it, and nothing where
/// none fills it or it cannot be asked.
pub(crate) async fn guard(ctx: &Ctx, fillers: &Fillers) -> Option<Box<dyn egress::Fills>> {
    let (filler, _) = fillers.filling(egress::CAPABILITY)?;
    match spoken(ctx, filler, egress::CAPABILITY, egress::MAJOR).await {
        Spoken::Over(adapter) => Some(Box::new(egress::Adapter(adapter))),
        Spoken::Unanswered => None,
        Spoken::Not => (filler.origin == Origin::Bundled || filler.stands_in()).then(|| {
            Box::new(Gluetun::new(
                ctx.seams.engine.clone(),
                ctx.settings.project.clone(),
                filler.id.clone(),
                ctx.settings.port_forward.enabled,
            )) as Box<dyn egress::Fills>
        }),
    }
}

/// A filler of `download.torrent` inside the guard.
pub(crate) struct Inside {
    /// The id its container runs under.
    pub service: String,
    /// The client over `download.torrent`, or nothing where it cannot be asked.
    pub client: Option<Box<dyn torrent::Fills>>,
}

/// Every filler of `download.torrent` that takes `network.via` the guard, itself or
/// through the upstream it fronts, in the order they are declared.
pub(crate) async fn inside(ctx: &Ctx, fillers: &Fillers) -> Vec<Inside> {
    let declared = declared_downloads(ctx, fillers).await;
    fillers
        .services()
        .filter(|filler| fills_torrent(filler) && takes_guard(fillers, filler))
        .map(|filler| Inside {
            service: filler.id.clone(),
            client: declared
                .iter()
                .find(|one| one.id == filler.id)
                .and_then(|one| one.target.as_ref())
                .and_then(DownloadTarget::torrent),
        })
        .collect()
}

fn fills_torrent(filler: &Filler) -> bool {
    filler
        .provides
        .iter()
        .any(|provided| provided == torrent::CAPABILITY)
}

fn takes_guard(fillers: &Fillers, filler: &Filler) -> bool {
    std::iter::once(filler)
        .chain(fillers.fronted_by(filler))
        .any(|one| one.via.as_deref() == Some(egress::CAPABILITY))
}

#[cfg(test)]
mod tests;
