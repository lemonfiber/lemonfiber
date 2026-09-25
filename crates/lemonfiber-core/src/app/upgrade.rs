//! Upgrading existing content to the chosen quality — a separate, cost-stated,
//! confirmed action.
//!
//! Changing a preset is forward-looking: it decides what is acquired next, never a
//! rewrite of what is on disk. Upgrading existing content is the opposite, and
//! costly — it re-acquires the library at the higher quality, potentially terabytes
//! of bandwidth and hours to days of time. So it is never a side effect of a preset
//! change: it is this explicit action, which states that cost and does nothing until
//! confirmed, and only then asks each \*arr to re-search what it already has for a
//! better release meeting the raised bar.

use super::targets::{project_directory, servarr_targets};
use super::Ctx;
use crate::doctor::credentials::Target;
use crate::error::{Diagnose, Problem};
use crate::model::{Triggered, UpgradeMedia, UpgradeReport};
use crate::ports::service::Maintenance;
use crate::recyclarr::Kind;

/// State the cost of upgrading existing content to the chosen quality, per media
/// type, and — only when confirmed — ask each resolution \*arr to re-search its
/// library for upgrades.
///
/// The cost is stated per media type because each carries its own preset: film at
/// maximum and television at space-saving are upgraded to different bars. Only the
/// resolution \*arrs present in the stack are covered; a music or index service is
/// not a resolution preset's concern, so it is left out rather than sent a command
/// it has no equivalent for.
pub(crate) async fn upgrade(ctx: &Ctx, confirm: bool) -> Result<UpgradeReport, Box<Problem>> {
    let selection = super::quality::recorded_selection(ctx);
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());

    // A confirmed run is the largest acquisition this product can start — the whole
    // library re-fetched — so it is the one that must not start onto a disk with no
    // room to write on. Asked only of a run that would act: stating the cost of an
    // upgrade is worth doing whatever the disk is doing.
    if confirm {
        super::space::admits(ctx).await?;
    }

    let mut media = Vec::new();
    for target in servarr_targets(&manifest.services, project.as_deref()) {
        let Some(kind) = Kind::for_section(&target.id) else {
            continue;
        };
        let preset = selection.for_type(kind.media_type());
        // Unconfirmed states the cost and touches nothing — the deliberate gate a
        // large, bandwidth-expensive operation sits behind.
        let outcome = if confirm {
            Some(trigger(ctx, kind, &target).await)
        } else {
            None
        };
        media.push(UpgradeMedia {
            media_type: kind.media_type().to_owned(),
            preset: preset.label().to_owned(),
            size_per_hour: preset.consequence().size_per_hour.to_owned(),
            outcome,
        });
    }
    Ok(UpgradeReport {
        confirmed: confirm,
        media,
    })
}

/// Ask one resolution \*arr to re-search its existing content for an upgrade, and read
/// what it said. The \*arr searches against its own current cutoff — whatever the last
/// applied preset set — so what actually upgrades is what sits below that bar.
async fn trigger(ctx: &Ctx, kind: Kind, target: &Target) -> Triggered {
    match target
        .open(&ctx.seams.http, ctx.seams.filesystem.as_ref())
        .await
    {
        None => Triggered::NotStarted,
        Some(service) => match service.run_command(kind.upgrade_command()).await {
            Ok(()) => Triggered::Started,
            Err(failure) => Triggered::Failed {
                detail: failure.to_string(),
            },
        },
    }
}

#[cfg(test)]
mod tests;
