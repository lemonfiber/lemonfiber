//! The curators on this machine, each asked over `library.curate` through the adapter in
//! front of it.
//!
//! A curator is the service that files media, as [`crate::seed::run::curators`] lists
//! it. A read is made of it through the adapter of its plugin, and a curator that adapter
//! cannot be asked through yet is still listed, so a report can say it was not asked.

use std::sync::Arc;

use lemonfiber_contract::capabilities::library::curate;

use crate::app::Ctx;
use crate::doctor::credentials::Target;
use crate::wiring::Fillers;

/// Every curator among `fillers`, each with the adapter it is asked through where it can
/// be asked.
pub(crate) async fn curator_targets(ctx: &Ctx, fillers: &Fillers) -> Vec<Target> {
    let mut targets = Vec::new();
    for curator in crate::seed::run::curators(fillers) {
        targets.push(Target {
            id: curator.id().to_owned(),
            name: curator.name().to_owned(),
            kind: curator.kind(),
            asked: curator
                .client(ctx)
                .await
                .map(|adapter| Arc::new(adapter) as Arc<dyn curate::Fills>),
        });
    }
    targets
}

/// The curator among `fillers` that files media as `id`, or nothing where none does — the
/// lookup a reversal makes, which knows the name of the service it has to reach and
/// nothing else about it.
pub(crate) async fn target_named(ctx: &Ctx, fillers: &Fillers, id: &str) -> Option<Target> {
    curator_targets(ctx, fillers)
        .await
        .into_iter()
        .find(|target| target.id == id)
}

#[cfg(test)]
mod tests;
