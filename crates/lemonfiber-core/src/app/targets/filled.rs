//! The media server as the core asks it: over the contracts the service filling it
//! speaks, or, where it speaks none, through the adapter this build holds for the
//! bundled server.
//!
//! The media server is whatever fills the identity source, and both of its capabilities
//! are asked of that one service. A service that speaks a capability's contract is asked
//! over it and no other way.

use std::sync::Arc;

use lemonfiber_contract::capabilities::identity::source;
use lemonfiber_contract::capabilities::media::serve;
use lemonfiber_contract::Contracted;
use lemonfiber_manifest::Manifest;

use crate::app::plugins::conformance;
use crate::app::Ctx;
use crate::wiring::{Filler, Fillers};

use super::media::{fillers_here, MediaServer, IDENTITY};

/// The media server's two capabilities, each as the service filling it answers.
#[derive(Clone)]
pub(crate) struct Media {
    /// Who the household is.
    pub identity: Arc<dyn source::Fills>,
    /// What the household holds and watches.
    pub serve: Arc<dyn serve::Fills>,
}

/// Both of the media server's capabilities, or nothing where either cannot be asked.
pub(crate) async fn media(ctx: &Ctx, manifest: &Manifest) -> Option<Media> {
    let fillers = fillers_here(ctx, manifest);
    Some(Media {
        identity: identified(ctx, &fillers).await?,
        serve: served(ctx, &fillers).await?,
    })
}

/// Who the household is, or nothing where the media server cannot be asked.
pub(crate) async fn identity(ctx: &Ctx, manifest: &Manifest) -> Option<Arc<dyn source::Fills>> {
    identified(ctx, &fillers_here(ctx, manifest)).await
}

/// What the household holds and watches, or nothing where the media server cannot be
/// asked.
pub(crate) async fn serving(ctx: &Ctx, manifest: &Manifest) -> Option<Arc<dyn serve::Fills>> {
    served(ctx, &fillers_here(ctx, manifest)).await
}

/// The service filling `media.serve`, where one does.
pub(crate) fn served_by(ctx: &Ctx, manifest: &Manifest) -> Option<String> {
    provider(&fillers_here(ctx, manifest), serve::CAPABILITY).map(|filler| filler.id.clone())
}

/// The media server, where it provides `capability`.
fn provider<'f>(fillers: &'f Fillers, capability: &str) -> Option<&'f Filler> {
    let (filler, _) = fillers.filling(IDENTITY)?;
    filler
        .provides
        .iter()
        .any(|provided| provided == capability)
        .then_some(filler)
}

async fn identified(ctx: &Ctx, fillers: &Fillers) -> Option<Arc<dyn source::Fills>> {
    provider(fillers, source::CAPABILITY)?;
    let server = MediaServer::of(fillers)?;
    Box::pin(server.identifying(ctx)).await
}

async fn served(ctx: &Ctx, fillers: &Fillers) -> Option<Arc<dyn serve::Fills>> {
    provider(fillers, serve::CAPABILITY)?;
    let server = MediaServer::of(fillers)?;
    Box::pin(server.administering(ctx)).await
}

/// How a service is asked for one capability's contract.
pub(crate) enum Spoken {
    /// Over the contract, every answer outside it kept against the plugin.
    Over(Contracted),
    /// Not at all: it speaks the contract and has an answer kept against it as outside
    /// it, or cannot be reached.
    Unanswered,
    /// It speaks no contract for the capability, so this build's own adapter is the way.
    Not,
}

/// How `filler` is asked for `capability` at `major`.
pub(crate) async fn spoken(ctx: &Ctx, filler: &Filler, capability: &str, major: u32) -> Spoken {
    if !filler.contracted(capability, major) {
        return Spoken::Not;
    }
    let Some(named) = filler
        .brought_by()
        .filter(|named| conformance::fills(ctx, named, capability))
    else {
        return Spoken::Unanswered;
    };
    let witness = conformance::witness(ctx, named, capability);
    match crate::plugin::reaching::filling(ctx, filler).await {
        Ok(adapter) => Spoken::Over(adapter.witnessed_by(witness)),
        Err(_) => Spoken::Unanswered,
    }
}

#[cfg(test)]
mod tests;
