//! Asking a service over its contract.
//!
//! Each reader here finds the service a read is made of among the fillers and hands back
//! the adapter that answers for it. Absent where that adapter cannot be asked yet, which
//! is a service still starting rather than a fault.

use std::sync::Arc;

use lemonfiber_contract::capabilities::indexer::search;
use lemonfiber_contract::capabilities::library::curate;
use lemonfiber_contract::capabilities::request::intake;

use crate::app::Ctx;
use crate::ports::service::UsenetAccounts;
use crate::wiring::{Filler, Fillers};

use crate::recyclarr::Kind;

use super::curators::curator_targets;
use super::downloads::{download_targets, DownloadTarget};
use super::filled::{spoken, Spoken};
use super::media::fillers_here;

/// The server the decline service acts on: the stack's own service its link by name
/// reaches, or the first-party service standing in for it, rather than whatever serves
/// identity, asked through the adapter of its own plugin that fronts it where one does,
/// and where that is one lemonfiber speaks to as a media server.
pub(crate) fn declined_server(fillers: &Fillers) -> Option<super::MediaServer> {
    let named = fillers.named_by(crate::app::invite::declining::SERVICE)?;
    super::MediaServer::at(fillers, fillers.fronting(named).unwrap_or(named))
}

/// One curator a read can be made against: the kind of video it files, the curator over
/// its contract, and the name a report calls it by.
pub(crate) struct OpenCurator {
    /// The service's display name, as a report names where a fact came from.
    pub name: String,
    /// Which of the two media services it is.
    pub kind: Kind,
    /// The curator, asked through the adapter in front of it.
    pub service: Arc<dyn curate::Fills>,
}

/// Every curator of video that can be asked, ready to be asked something.
///
/// One whose adapter cannot be asked yet is left out. That is deliberately not a failed
/// read: a service still coming up holds nothing to report, so its absence understates
/// nothing — the convention every caller here follows, stated once rather than
/// re-derived at each of them.
pub(crate) async fn open_curators(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
) -> Vec<OpenCurator> {
    let fillers = fillers_here(ctx, manifest);
    let mut open = Vec::new();
    for target in curator_targets(ctx, &fillers).await {
        if let (Some(kind), Some(service)) = (target.kind, target.asked) {
            open.push(OpenCurator {
                name: target.name,
                kind,
                service,
            });
        }
    }
    open
}

/// What reading the household's requests needs: the request service, asked as its
/// owner, whose reads see every member's requests.
pub(crate) struct HouseholdAccess {
    /// The request service.
    pub requests: Arc<dyn intake::Fills>,
}

/// The request service to read the household from, or nothing where there is no media
/// server for it to authenticate the household against or [`requests_from`] finds none.
///
/// The household view treats either as nothing to report rather than a fault: a stack
/// without a request service has no household requests, and one not yet set up has
/// nobody to have asked for anything.
pub(crate) async fn household_requests(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
) -> Option<HouseholdAccess> {
    let fillers = fillers_here(ctx, manifest);
    super::media::MediaServer::of(&fillers)?;
    requests_from(ctx, &fillers).await
}

/// Whatever among `fillers` fills `request.intake`, asked over its contract: nothing
/// where nothing here fills it or it cannot be asked over it.
pub(crate) async fn requests_from(ctx: &Ctx, fillers: &Fillers) -> Option<HouseholdAccess> {
    let requests = requests_as_owner(ctx, request_service(fillers)?).await?;
    Some(HouseholdAccess { requests })
}

/// The service filling `request.intake` among `fillers`, as the ask for it settles or as
/// the one service providing it.
pub(crate) fn request_service(fillers: &Fillers) -> Option<&Filler> {
    fillers
        .filling(intake::CAPABILITY)
        .map(|(filler, _)| filler)
}

/// The request service `filler` is, asked as its owner over `request.intake`: nothing
/// where it does not speak the contract or cannot be asked over it.
pub(crate) async fn requests_as_owner(
    ctx: &Ctx,
    filler: &Filler,
) -> Option<Arc<dyn intake::Fills>> {
    match spoken(ctx, filler, intake::CAPABILITY, intake::MAJOR).await {
        Spoken::Over(adapter) => Some(Arc::new(intake::Adapter(adapter))),
        Spoken::Unanswered | Spoken::Not => None,
    }
}

/// The first Usenet download client among `fillers`, as a reader of the accounts behind
/// it.
///
/// Nothing where there is no Usenet client, or where no adapter of its plugin can be asked
/// for it.
pub(crate) async fn usenet_client(ctx: &Ctx, fillers: &Fillers) -> Option<Arc<dyn UsenetAccounts>> {
    let client: Box<dyn UsenetAccounts> = download_targets(ctx, fillers)
        .await
        .iter()
        .find_map(DownloadTarget::usenet)?;
    Some(Arc::from(client))
}

/// The indexer the stack searches through, asked over `indexer.search`: the service
/// filling it as the stack settles it, and nothing where nothing here fills it or it
/// cannot be asked over it.
///
/// It is the one that knows how the indexers have been behaving.
pub(crate) async fn indexer_aggregator(ctx: &Ctx, fillers: &Fillers) -> Option<search::Adapter> {
    let (filler, _) = fillers.filling(search::CAPABILITY)?;
    match spoken(ctx, filler, search::CAPABILITY, search::MAJOR).await {
        Spoken::Over(adapter) => Some(search::Adapter(adapter)),
        Spoken::Unanswered | Spoken::Not => None,
    }
}

#[cfg(test)]
mod tests;
