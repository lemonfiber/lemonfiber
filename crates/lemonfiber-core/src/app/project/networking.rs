//! The project's networks, created through the engine before a start writes the project.
//!
//! Each network the engine already holds for the project keeps the subnet it holds there
//! where that is still free, so only a network it does not hold searches for one. Every
//! network of the project the topology no longer declares is taken away first, and so is
//! every one it declares that the engine holds in another shape, so none holds an address a
//! network is created with. Then each network the topology declares and the engine does not
//! hold as the project gives it is created under its engine name, labelled with the project
//! and its declared name, in the shape the project gives it. One the engine already holds
//! under that name, labelled and shaped so, is kept with every container on it, whichever
//! program created it. Each service whose container is on a network created again is named,
//! so a start creates it again. Then each one's subnet, gateway and id are read back and kept
//! where the project reads them. A rehearsal creates, takes away and keeps nothing, and names
//! what a start would create and create again.

use lemonfiber_ports::docker::{Network, Wanted};

use super::super::Ctx;
use crate::error::codes::stack::NETWORK_NOT_MADE;
use crate::error::{Problem, Remedy};
use crate::fence::{Answered, NETWORKS_HELD};
use crate::project::document::{named, shaped};
use crate::topology::{block, Topology};

/// The networks a start created and created again, by the engine's names for them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Networking {
    /// Each network of the project the project no longer declares, taken away before any
    /// was created, so none holds an address a network is created with.
    pub(crate) swept: Vec<String>,
    /// Each network created where the engine held none of its name.
    pub(crate) created: Vec<String>,
    /// Each network taken away and created again, the engine having held it in another
    /// shape: every one taken away before any is created, since a network's subnet moves
    /// where another came to its slot first.
    pub(crate) recreated: Vec<String>,
    /// Each service whose container was on a network created again, which a start creates
    /// again.
    pub(crate) displaced: Vec<String>,
}

/// Create each network `topology` declares that the engine does not hold as the project
/// gives it, and keep what the engine answers for each.
///
/// # Errors
///
/// Where the engine's networks cannot be listed, or one cannot be taken away or created.
pub(crate) async fn ensured(ctx: &Ctx, topology: &Topology) -> Result<Networking, Box<Problem>> {
    let project = ctx.settings.project.as_str();
    let mut networking = Networking::default();
    let sweep = if ctx.dry_run {
        super::Sweep::default()
    } else {
        super::swept(ctx, topology).await
    };
    sweep.taken_away(ctx).await;
    let held = listed(ctx, project).await?;
    networking.swept = sweep
        .networks
        .into_iter()
        .filter(|name| !held.iter().any(|one| one.name == *name))
        .collect();
    let topology = topology.clone().holding(
        &held
            .iter()
            .filter_map(|one| Some((one.declared.clone(), block(one.shape.subnet.as_deref()?)?)))
            .collect(),
    );
    let mut wanted = Vec::new();
    for (declared, network) in &topology.networks {
        let name = named(project, declared);
        let shape = shaped(network);
        let found = held.iter().find(|one| one.name == name);
        if found.is_some_and(|one| one.declared == *declared && one.shape == shape) {
            continue;
        }
        if found.is_some() {
            networking.recreated.push(name.clone());
        } else {
            networking.created.push(name.clone());
        }
        wanted.push((name, declared, shape));
    }
    if ctx.dry_run {
        return Ok(networking);
    }
    if !networking.recreated.is_empty() {
        networking.displaced = displaced(ctx, project, &networking.recreated).await;
    }
    for name in &networking.recreated {
        ctx.seams
            .networks
            .removed(name)
            .await
            .map_err(|failure| unmade(name, &failure.to_string()))?;
    }
    for (name, declared, shape) in &wanted {
        ctx.seams
            .networks
            .created(Wanted {
                name,
                project,
                declared,
                shape,
            })
            .await
            .map_err(|failure| unmade(name, &failure.to_string()))?;
    }
    let answered = listed(ctx, project).await?;
    let text: String = topology
        .networks
        .keys()
        .filter_map(|declared| {
            let name = named(project, declared);
            let network = answered.iter().find(|one| one.name == name)?;
            Some(read(network)?.line(declared))
        })
        .collect();
    if let Some(why) = super::directory(ctx)
        .and_then(|directory| super::fencing::kept(&directory.join(NETWORKS_HELD), &text).err())
    {
        return Err(unmade(NETWORKS_HELD, &why));
    }
    Ok(networking)
}

/// Take away every network `topology` declares that the engine holds for the project, once
/// a teardown left no container of the project behind; say each that would not go and why.
pub(crate) async fn taken_down(ctx: &Ctx, topology: &Topology) {
    let project = ctx.settings.project.as_str();
    if !ctx
        .seams
        .engine
        .list(project)
        .await
        .is_ok_and(|left| left.is_empty())
    {
        return;
    }
    let held = ctx
        .seams
        .networks
        .networks(project)
        .await
        .unwrap_or_default();
    for network in held
        .iter()
        .filter(|network| topology.networks.contains_key(&network.declared))
    {
        if let Err(why) = ctx.seams.networks.removed(&network.name).await {
            ctx.narrator
                .say(&format!(
                    "a network of the project could not be taken away: {}: {why}",
                    network.name
                ))
                .await;
        }
    }
}

/// Each service of `project` whose container is on one of `recreated`.
async fn displaced(ctx: &Ctx, project: &str, recreated: &[String]) -> Vec<String> {
    ctx.seams
        .engine
        .list(project)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|container| {
            container
                .networks
                .iter()
                .any(|network| recreated.contains(network))
        })
        .map(|container| container.service)
        .collect()
}

/// Every network the engine holds for `project`.
async fn listed(ctx: &Ctx, project: &str) -> Result<Vec<Network>, Box<Problem>> {
    ctx.seams
        .networks
        .networks(project)
        .await
        .map_err(|failure| unmade(project, &failure.to_string()))
}

/// `network` as the engine answered it, where its subnet and gateway read whole.
fn read(network: &Network) -> Option<Answered> {
    Answered::of(
        network.shape.subnet.as_deref()?,
        network.gateway.as_deref()?,
        &network.id,
    )
}

/// The refusal of a start whose network `name` could not be made as the project gives it.
fn unmade(name: &str, why: &str) -> Box<Problem> {
    Box::new(
        Problem::new(
            NETWORK_NOT_MADE,
            format!("The network {name} could not be made as the project gives it, so nothing was started"),
            "lemonfiber creates every network of the project before a start, and a service \
             started now would start on a network the project does not describe.",
            Remedy::new("Check the container engine is running, then start again"),
        )
        .with_detail(why.to_owned()),
    )
}

#[cfg(test)]
mod tests;
