//! The checks a diagnosis builds only when it runs them, and the readings each is
//! built from.
//!
//! Apart from the assembly because each of these is a reading as well as a check,
//! and a reading made when the register is assembled happens for every check whether
//! or not it is asked for — and outside every check's budget. See
//! [`crate::doctor::deferred`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::app::targets::{committed_bytes, host_fillers};
use crate::app::Ctx;
use crate::doctor::deferred::Deferred;
use crate::doctor::providers::ProvidersCheck;
use crate::doctor::storage::StorageCheck;
use crate::doctor::vpn::{budget_for, VpnCheck};
use crate::doctor::wiring::WiringCheck;
use crate::doctor::{Category, Check, CHECK_BUDGET, FILESYSTEM_BUDGET};
use crate::ports::service::{Indexers, UsenetAccounts};

use super::Stack;

/// Whether what is meant to leave the house through the tunnel actually does, built
/// when it runs.
pub(super) fn tunnel(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    project: Option<&Path>,
    disruptive: bool,
) -> Deferred {
    let (ctx, manifest) = (ctx.clone(), manifest.clone());
    let project = project.map(Path::to_path_buf);
    Deferred::new(Category::Vpn, budget_for(disruptive), move || {
        let (ctx, manifest, project) = (ctx.clone(), manifest.clone(), project.clone());
        async move {
            Box::new(tunnelled(&ctx, &manifest, project.as_deref(), disruptive).await)
                as Box<dyn Check>
        }
    })
}

/// What the accounts underneath the stack have left, built when it runs.
pub(super) fn providing(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    project: Option<&Path>,
) -> Deferred {
    let (ctx, manifest) = (ctx.clone(), manifest.clone());
    let project: Option<PathBuf> = project.map(Path::to_path_buf);
    Deferred::new(Category::Providers, CHECK_BUDGET, move || {
        let (ctx, manifest, project) = (ctx.clone(), manifest.clone(), project.clone());
        async move {
            Box::new(provider_accounts(&ctx, &manifest, project.as_deref()).await) as Box<dyn Check>
        }
    })
}

/// Whether the data location can hold the library, built when it runs.
///
/// What the download clients still have to write is read as the first part of the
/// run, so the free-space finding projects exhaustion from the queue rather than
/// only warning on a floor. Resolved from the same running-stack services the
/// credentials check reaches; a client that will not answer contributes nothing, so
/// a stack whose clients are all quiet reads as zero committed and the finding guards
/// the raw free space.
///
/// The mounts are read here rather than inside the check, for the reason every other
/// reading is: a check holds the seam it looks through, and the stack's own files are
/// not reached through one. What the storage check does with them is report the half
/// of the hardlink question its probe cannot see — a fork that splits the data
/// location between two mounts, where imports copy however well the host links.
pub(super) fn stored(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    project: Option<&Path>,
) -> Deferred {
    let (ctx, manifest) = (ctx.clone(), manifest.clone());
    let project = project.map(Path::to_path_buf);
    Deferred::new(Category::Storage, FILESYSTEM_BUDGET, move || {
        let (ctx, manifest, project) = (ctx.clone(), manifest.clone(), project.clone());
        async move {
            let fillers = host_fillers(&ctx, &manifest, project.as_deref());
            let committed = committed_bytes(&ctx, &fillers).await;
            Box::new(StorageCheck::new(
                ctx.seams.filesystem.clone(),
                ctx.settings.data_root.clone(),
                ctx.settings.storage_state.clone(),
                ctx.environment,
                ctx.settings.service_user,
                Some(committed),
                ctx.stack.crowded_mounts(),
            )) as Box<dyn Check>
        }
    })
}

/// Whether each download client still files where lemonfiber wired it, built when it
/// runs.
///
/// The one field an operator and lemonfiber both write, so the only place a fix could
/// write over somebody's own change. Read-only here: it says which side of the field
/// moved, and the repair it hands back refuses to move the operator's.
pub(super) fn wired(ctx: &Ctx, stack: &Stack, project: Option<&Path>) -> Deferred {
    let (ctx, manifest, installed) = (ctx.clone(), stack.manifest.clone(), stack.installed.clone());
    let project = project.map(Path::to_path_buf);
    Deferred::new(Category::Config, CHECK_BUDGET, move || {
        let (ctx, manifest, installed) = (ctx.clone(), manifest.clone(), installed.clone());
        let project = project.clone();
        async move {
            let wirings =
                crate::seed::run::managed_wirings(&ctx, &manifest, &installed, project.as_deref())
                    .await;
            Box::new(WiringCheck::new(
                ctx.seams.http.clone(),
                ctx.seams.filesystem.clone(),
                wirings,
                ctx.stamp(),
            )) as Box<dyn Check>
        }
    })
}

/// Whether what is meant to leave the house through the tunnel actually does.
///
/// Built apart from the assembly for the same reason the other three built apart from
/// it are: asking this question takes more lines than any of the checks beside it, and
/// an assembly longer than a reader holds in one go is one somebody adds a check to
/// twice. What it needs that a check may not reach for itself — the port a client says
/// it is listening on, and the client it would be corrected through — is read here,
/// because this check speaks to containers and those are a service's own business.
async fn tunnelled(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    project: Option<&Path>,
    disruptive: bool,
) -> VpnCheck {
    VpnCheck::new(
        ctx.seams.engine.clone(),
        ctx.settings.project.clone(),
        manifest,
        crate::doctor::vpn::Asked {
            protocols: ctx.settings.protocols,
            echo: ctx.settings.ip_echo.clone(),
            listening: crate::app::forwarding::listening_port(ctx, manifest, project).await,
            port_forward: ctx.settings.port_forward.clone(),
            disruptive,
            client: crate::app::targets::forwarded_client(
                ctx,
                &crate::app::targets::download_targets(ctx, &host_fillers(ctx, manifest, project))
                    .await,
            ),
        },
    )
}

/// What the accounts underneath the stack have left, read from the services that use
/// them.
///
/// The download client pulls through the Usenet accounts and the aggregator queries the
/// indexers, and both keep their own records — so this costs the providers nothing. A
/// check that spent the quota it measures would help cause the outage it is there to
/// warn about.
async fn provider_accounts(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    project: Option<&Path>,
) -> ProvidersCheck {
    let services = &manifest.services;
    ProvidersCheck::new(
        crate::app::targets::usenet_client(ctx, &host_fillers(ctx, manifest, project))
            .await
            .map(|client| Arc::new(client) as Arc<dyn UsenetAccounts>),
        crate::app::targets::indexer_aggregator(ctx, services, project)
            .await
            .map(|aggregator| Arc::new(aggregator) as Arc<dyn Indexers>),
        ctx.today(),
        ctx.seams.clock.now(),
    )
}

#[cfg(test)]
mod tests;
