//! The networks the engine holds for the project that the project no longer declares.
//!
//! Compose never takes away a network its file stopped declaring, and one left holding a
//! subnet the project now fixes elsewhere stops the start that would create the new one.
//! So before a start, every network labelled with the project and declared neither by it
//! nor by the operator's overlay is disconnected from whatever is still on it and taken
//! away; a rehearsal names them and takes nothing.

use crate::topology::Topology;

use super::super::Ctx;

/// The networks a start would take away, by the engine's names for them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Sweep {
    /// Each one, by the engine's name.
    pub(crate) networks: Vec<String>,
}

/// The networks the engine holds for this project that neither `topology` nor the
/// operator's overlay declares. An engine that will not say holds none this could name.
pub(crate) async fn swept(ctx: &Ctx, topology: &Topology) -> Sweep {
    let held = ctx
        .seams
        .networks
        .networks(&ctx.settings.project)
        .await
        .unwrap_or_default();
    let theirs = match ctx.settings.env_file.as_deref() {
        Some(env) => ctx
            .seams
            .filesystem
            .read(&crate::overlay::at(env))
            .await
            .map(|text| crate::project::overlay::networks(&text))
            .unwrap_or_default(),
        None => Vec::new(),
    };
    Sweep {
        networks: held
            .into_iter()
            .filter(|network| {
                !topology.networks.contains_key(&network.declared)
                    && !theirs.contains(&network.declared)
            })
            .map(|network| network.name)
            .collect(),
    }
}

impl Sweep {
    /// Take every one of them away, saying each that would not go and why.
    pub(crate) async fn taken_away(&self, ctx: &Ctx) {
        for network in &self.networks {
            if let Err(why) = ctx.seams.networks.removed(network).await {
                ctx.narrator
                    .say(&format!(
                        "a network the project no longer declares could not be taken away: \
                         {network}: {why}"
                    ))
                    .await;
            }
        }
    }
}
