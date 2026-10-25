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
use crate::doctor::declining::{Dating, Decline, DeclineKeyCheck, KeyDates};
use crate::doctor::deferred::Deferred;
use crate::doctor::owned::{OwnedCheck, Part};
use crate::doctor::providers::ProvidersCheck;
use crate::doctor::seasons::SeasonsCheck;
use crate::doctor::storage::StorageCheck;
use crate::doctor::vpn::{budget_for, VpnCheck};
use crate::doctor::wiring::WiringCheck;
use crate::doctor::{Category, Check, CHECK_BUDGET, FILESYSTEM_BUDGET};
use crate::ports::service::{Indexers, Ownership};

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

/// Whether the people in the house will hear back about what they asked for, built when
/// it runs, so the request service is asked over its contract where it speaks it.
pub(super) fn telling(ctx: &Ctx, manifest: &lemonfiber_manifest::Manifest) -> Deferred {
    let (ctx, manifest) = (ctx.clone(), manifest.clone());
    Deferred::new(Category::Services, CHECK_BUDGET, move || {
        let (ctx, manifest) = (ctx.clone(), manifest.clone());
        async move {
            let fillers = crate::app::targets::fillers_here(&ctx, &manifest);
            let (requests, recorded) = crate::seed::run::managed_telling(&ctx, &fillers).await;
            Box::new(crate::doctor::telling::TellingCheck::new(
                requests, recorded,
            )) as Box<dyn Check>
        }
    })
}

/// Whether the decline service's key was used for anything the service did not record,
/// built when it runs, so the media server that dates the key is asked over its contract
/// where it speaks it.
pub(super) fn declined(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    project: Option<&Path>,
) -> Deferred {
    let (ctx, manifest) = (ctx.clone(), manifest.clone());
    let project: Option<PathBuf> = project.map(Path::to_path_buf);
    Deferred::new(Category::Services, CHECK_BUDGET, move || {
        let (ctx, manifest, project) = (ctx.clone(), manifest.clone(), project.clone());
        async move {
            Box::new(DeclineKeyCheck::new(
                ctx.seams.filesystem.clone(),
                decline(&ctx, &manifest, project.as_deref()).await,
            )) as Box<dyn Check>
        }
    })
}

/// The decline service's two records and the media server that dates its key, where the
/// stack runs one.
async fn decline(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    project: Option<&Path>,
) -> Option<Decline> {
    use lemonfiber_sidecar::decline::File;
    let project =
        project.filter(|_| crate::app::invite::declining::service(&manifest.services).is_some())?;
    let fillers = crate::app::targets::fillers_here(ctx, manifest);
    let keys = match crate::app::targets::declined_server(&fillers) {
        Some(server) => server.administering(ctx).await,
        None => None,
    };
    Some(Decline {
        refusals: crate::app::invite::declining::path(project, File::Refusals),
        lapses: crate::app::invite::declining::path(project, File::Lapses),
        keys: keys.map(|server| Arc::new(Dating(server)) as Arc<dyn KeyDates>),
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
/// reading is: a check holds the seam it looks through, and the project and the
/// operator's overlay are not reached through one. What the storage check does with them
/// is report the half of the hardlink question its probe cannot see — an overlay that
/// splits the data location between two mounts, where imports copy however well the
/// host links.
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
            Box::new(
                StorageCheck::new(
                    ctx.seams.filesystem.clone(),
                    ctx.settings.data_root.clone(),
                    ctx.settings.storage_state.clone(),
                    ctx.environment,
                    ctx.settings.service_user,
                    Some(committed),
                    crate::app::project::crowded(&ctx),
                )
                .on(ctx.settings.nfs.clone())
                .refusing(crate::app::project::refused_root(&ctx)),
            ) as Box<dyn Check>
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

/// Who holds every part that belongs to whoever claims it first, built when it runs, so
/// each is asked over its contract.
pub(super) fn owned(ctx: &Ctx, manifest: &lemonfiber_manifest::Manifest) -> Deferred {
    let (ctx, manifest) = (ctx.clone(), manifest.clone());
    Deferred::new(Category::Config, CHECK_BUDGET, move || {
        let (ctx, manifest) = (ctx.clone(), manifest.clone());
        async move {
            let fillers = crate::app::targets::fillers_here(&ctx, &manifest);
            let parts = crate::seed::run::owning::parts(&ctx, &fillers)
                .await
                .into_iter()
                .map(|(filler, part)| {
                    (
                        Part {
                            id: filler.id.clone(),
                            name: filler.name.clone(),
                            secrets: filler.upstream.clone(),
                        },
                        part.map(|part| Arc::new(part) as Arc<dyn Ownership>),
                    )
                })
                .collect();
            Box::new(OwnedCheck::new(parts)) as Box<dyn Check>
        }
    })
}

/// Whether every series the media server holds opens onto its seasons, built when it runs.
pub(super) fn seasons(ctx: &Ctx, manifest: &lemonfiber_manifest::Manifest) -> Deferred {
    let (ctx, manifest) = (ctx.clone(), manifest.clone());
    Deferred::new(Category::Services, CHECK_BUDGET, move || {
        let (ctx, manifest) = (ctx.clone(), manifest.clone());
        async move {
            let service = crate::app::targets::served_by(&ctx, &manifest);
            let served = crate::app::targets::serving(&ctx, &manifest).await;
            Box::new(SeasonsCheck::new(service, served)) as Box<dyn Check>
        }
    })
}

/// Whether what is meant to leave the house through the tunnel actually does.
///
/// Built apart from the assembly for the same reason the other three built apart from
/// it are: asking this question takes more lines than any of the checks beside it, and
/// an assembly longer than a reader holds in one go is one somebody adds a check to
/// twice. What it needs that a check may not reach for itself — the port a client says
/// it is listening on — is read here, because this check speaks to containers and that
/// is a service's own business.
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
        crate::app::targets::usenet_client(ctx, &host_fillers(ctx, manifest, project)).await,
        crate::app::targets::indexer_aggregator(ctx, services, project)
            .await
            .map(|aggregator| Arc::new(aggregator) as Arc<dyn Indexers>),
        ctx.today(),
        ctx.seams.clock.now(),
    )
}

#[cfg(test)]
mod tests;
