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
use crate::jellyfin::Jellyfin;
use crate::wiring::Fillers;

use super::media::{fillers_here, settled, MediaServer};

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

async fn identified(ctx: &Ctx, fillers: &Fillers) -> Option<Arc<dyn source::Fills>> {
    Some(
        match reached(ctx, fillers, source::CAPABILITY, source::MAJOR).await? {
            Reached::Contracted(adapter) => Arc::new(source::Adapter(adapter)),
            Reached::Bundled(server) => Arc::new(server),
        },
    )
}

async fn served(ctx: &Ctx, fillers: &Fillers) -> Option<Arc<dyn serve::Fills>> {
    Some(
        match reached(ctx, fillers, serve::CAPABILITY, serve::MAJOR).await? {
            Reached::Contracted(adapter) => Arc::new(serve::Adapter(adapter)),
            Reached::Bundled(server) => Arc::new(server),
        },
    )
}

/// How the media server is asked for one capability.
enum Reached {
    /// Over the contract it speaks.
    Contracted(Contracted),
    /// Through this build's adapter for the bundled server, as its administrator.
    Bundled(Jellyfin),
}

/// How the service filling the identity source is asked for `capability`.
///
/// Nothing where it does not provide `capability`, where it speaks the contract and is
/// a plugin with an answer kept against it as outside that contract, or cannot be
/// reached, and where it speaks none and is not a server this build holds an adapter
/// and a recorded password for. Every answer outside the contract is kept against the
/// plugin.
async fn reached(ctx: &Ctx, fillers: &Fillers, capability: &str, major: u32) -> Option<Reached> {
    let (filler, _) = settled(fillers)?;
    if !filler
        .provides
        .iter()
        .any(|provided| provided == capability)
    {
        return None;
    }
    if filler.contracted(capability, major) {
        let named = filler.brought_by()?;
        if !conformance::fills(ctx, named, capability) {
            return None;
        }
        let witness = conformance::witness(ctx, named, capability);
        return crate::plugin::reaching::filling(ctx, filler)
            .await
            .ok()
            .map(|adapter| Reached::Contracted(adapter.witnessed_by(witness)));
    }
    let server = MediaServer::of(fillers)?.administered(ctx)?;
    Some(Reached::Bundled(
        server.remembering(Arc::clone(&ctx.sessions)),
    ))
}

#[cfg(test)]
mod tests;
