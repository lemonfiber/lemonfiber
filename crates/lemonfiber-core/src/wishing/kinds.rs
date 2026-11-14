//! Which kinds a member may ask for now, each with the units it is asked for in.

use super::{not_working, reached};
use crate::app::Ctx;
use crate::collected::Medium;
use crate::error::Problem;
use crate::model::{AskableKind, AskableKindsReport, TitleKind, TitleUnit};
use crate::ports::media::Kind;
use crate::ports::service::Left;

/// The kinds the member `named` may ask for now: series and films where a curator of
/// them runs, music and books in the units their curator answers for now, each left out
/// where the member's allowance takes none of it.
///
/// # Errors
///
/// The refusals a search answers with for who is named and what answers for them.
pub(super) async fn kinds(ctx: &Ctx, named: &str) -> Result<AskableKindsReport, Box<Problem>> {
    let reached = reached(ctx, named).await?;
    let theirs = crate::household::run::allowance_of(
        reached.requests.as_ref(),
        &reached.member,
        ctx.stamp(),
    )
    .await
    .map_err(|_| not_working())?;
    let mut kinds: Vec<AskableKind> = reached
        .kinds
        .iter()
        .filter(|kind| {
            takes_any(match kind {
                Kind::Tv => theirs.headroom.television,
                Kind::Movies => theirs.headroom.films,
            })
        })
        .map(|kind| AskableKind {
            kind: TitleKind::from(*kind),
            units: Vec::new(),
        })
        .collect();
    if takes_any(theirs.headroom.films) {
        for medium in Medium::ALL {
            let units = answered(ctx, medium).await;
            if !units.is_empty() {
                kinds.push(AskableKind {
                    kind: TitleKind::from(medium),
                    units,
                });
            }
        }
    }
    Ok(AskableKindsReport {
        member: reached.member.name,
        kinds,
    })
}

/// Whether an allowance takes any ask at all.
fn takes_any(left: Left) -> bool {
    left.limit != Some(0)
}

/// The units of `medium` its curator answers for now: none where no curator of it is
/// settled or it does not answer.
async fn answered(ctx: &Ctx, medium: Medium) -> Vec<TitleUnit> {
    let Some(curator) = crate::collected::run::collector(ctx, medium).await else {
        return Vec::new();
    };
    curator
        .units()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(TitleUnit::from)
        .collect()
}

#[cfg(test)]
mod tests;
