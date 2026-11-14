//! Upgrading existing content to the chosen quality — a separate, cost-stated,
//! confirmed action.
//!
//! Changing a preset is forward-looking: it decides what is acquired next, never a
//! rewrite of what is on disk. Upgrading existing content is the opposite, and
//! costly — it re-acquires the library at the higher quality, potentially terabytes
//! of bandwidth and hours to days of time. So it is never a side effect of a preset
//! change: it is this explicit action, which states that cost and does nothing until
//! confirmed, and only then asks each curator to re-search what it already has for a
//! better release meeting the raised bar.

use super::targets::{curator_targets, fillers_here};
use super::Ctx;
use crate::doctor::credentials::Target;
use crate::error::{Diagnose, Problem};
use crate::model::{Total, Triggered, UpgradeMedia, UpgradeReport};
use crate::ports::media::TitleKind;
use crate::ports::service::Failure;
use crate::quality::Selection;

/// Seconds in an hour, which a rate per hour is divided by.
const AN_HOUR: u128 = 3_600;

/// State the cost of upgrading existing content to the chosen quality, per media
/// type, and — only when confirmed — ask each curator to re-search its library for
/// upgrades.
///
/// The cost is stated per media type because each carries its own preset or format: film
/// at maximum and television at space-saving are upgraded to different bars. It is what
/// the curator holds below its cutoff, re-fetched at that bar. Books have no quality to
/// upgrade to, so a curator of books is left out.
pub(crate) async fn upgrade(ctx: &Ctx, confirm: bool) -> Result<UpgradeReport, Box<Problem>> {
    let selection = super::quality::recorded_selection(ctx);
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    // A confirmed run is the largest acquisition this product can start — the whole
    // library re-fetched — so it is the one that must not start onto a disk with no
    // room to write on. Asked only of a run that would act: stating the cost of an
    // upgrade is worth doing whatever the disk is doing.
    if confirm {
        super::space::admits(ctx).await?;
    }

    let mut media = Vec::new();
    for target in curator_targets(ctx, &fillers_here(ctx, &manifest)).await {
        let Some((kind, preset, rate)) = target.kind.and_then(|kind| bar(&selection, kind)) else {
            continue;
        };
        // Unconfirmed states the cost and touches nothing — the deliberate gate a
        // large, bandwidth-expensive operation sits behind.
        let outcome = if confirm {
            Some(trigger(kind, &target).await)
        } else {
            None
        };
        media.push(UpgradeMedia {
            media_type: kind.media_type().to_owned(),
            preset: preset.to_owned(),
            total: total(kind, rate, &target).await,
            outcome,
        });
    }
    Ok(UpgradeReport {
        rehearsed: false,
        confirmed: confirm,
        media,
    })
}

/// The bar `kind` is upgraded to, by its label, and the bytes an hour of it takes there;
/// nothing for books, which have none.
fn bar(selection: &Selection, kind: TitleKind) -> Option<(TitleKind, &'static str, u64)> {
    match kind.video() {
        Some(video) => {
            let preset = selection.for_type(video.media_type());
            Some((kind, preset.label(), preset.bytes_per_hour()))
        }
        None if kind == TitleKind::Music => {
            let format = selection.music();
            Some((kind, format.label(), format.bytes_per_hour()))
        }
        None => None,
    }
}

/// What re-fetching what `target` holds of `kind` below its cutoff takes at `rate` bytes an
/// hour, or why that could not be worked out.
async fn total(kind: TitleKind, rate: u64, target: &Target) -> Total {
    let Some(service) = &target.asked else {
        return Total::Unknown(format!(
            "{} has not finished starting, so what it holds below its cutoff could not be read",
            target.name
        ));
    };
    match service.cutoff_unmet(kind).await {
        Ok(below) => Total::Bytes(
            u64::try_from(u128::from(below.seconds) * u128::from(rate) / AN_HOUR)
                .unwrap_or(u64::MAX),
        ),
        Err(Failure::Unsupported { .. }) => Total::Unknown(format!(
            "{}'s adapter does not say what it holds below its cutoff",
            target.name
        )),
        Err(failure) => Total::Unknown(format!(
            "what {} holds below its cutoff could not be read: {failure}",
            target.name
        )),
    }
}

/// Ask one curator to re-search its existing content for an upgrade, and read what it said.
/// The curator searches against its own current cutoff — whatever the last applied preset
/// set — so what actually upgrades is what sits below that bar.
async fn trigger(kind: TitleKind, target: &Target) -> Triggered {
    match &target.asked {
        None => Triggered::NotStarted,
        Some(service) => match service.search_upgrades(kind).await {
            Ok(()) => Triggered::Started,
            Err(failure) => Triggered::Failed {
                detail: failure.to_string(),
            },
        },
    }
}

#[cfg(test)]
mod tests;
