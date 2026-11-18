//! The services a start created again, each waited on until it answers.
//!
//! A container the program running the project created again is running before the
//! service in it answers, and the service it fronts or the one asking it may be asked
//! straight after the start. So the containers are read before the start runs and again
//! once its services settle, and every service whose container is another one is asked
//! through the adapter in front of it until it says it answers.

use std::collections::{BTreeMap, BTreeSet};

use super::super::Ctx;
use crate::ports::service::Failure;

/// The engine's id for the container of each service of the project, as it stood before a
/// start ran.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Containers(BTreeMap<String, String>);

impl Containers {
    /// Each service's container as the engine holds it now: none where it cannot say.
    pub(crate) async fn read(ctx: &Ctx) -> Self {
        Self(
            ctx.seams
                .engine
                .list(&ctx.settings.project)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|container| (container.service, container.id))
                .collect(),
        )
    }

    /// Each service whose container `now` holds is another than the one held here.
    fn recreated(&self, now: &Self) -> BTreeSet<String> {
        now.0
            .iter()
            .filter(|(service, id)| self.0.get(*service).is_some_and(|before| before != *id))
            .map(|(service, _)| service.clone())
            .collect()
    }
}

/// Wait, within a restart's patience each, for every service whose container is another
/// than `before` held to answer, asked through the adapter in front of it or the one it is;
/// a service asked over no contract is not waited on.
pub(crate) async fn answered(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    before: &Containers,
) {
    let recreated = before.recreated(&Containers::read(ctx).await);
    if recreated.is_empty() {
        return;
    }
    let project = super::super::project::directory(ctx);
    let fillers = super::super::targets::host_fillers(ctx, manifest, project.as_deref());
    let speakers: BTreeSet<&str> = fillers
        .services()
        .filter(|filler| recreated.contains(&filler.id))
        .map(|filler| fillers.fronting(filler).unwrap_or(filler).id.as_str())
        .collect();
    for speaker in fillers
        .services()
        .filter(|filler| speakers.contains(filler.id.as_str()))
    {
        let Ok(adapter) = crate::plugin::reaching::filling(ctx, speaker).await else {
            continue;
        };
        let ready = |answer: &Result<bool, Failure>| matches!(answer, Ok(true));
        if !ready(&adapter.ready().await) {
            let _ = crate::patience::RESTART
                .until(|| adapter.ready(), ready)
                .await;
        }
    }
}

#[cfg(test)]
mod tests;
