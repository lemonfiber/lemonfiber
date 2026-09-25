//! What taking an existing stack over would come to, and what nothing carries across.
//!
//! An \*arr migrates its database forward on first start, and the binary that did so is
//! the oldest that can open it afterwards. So the version standing on an existing
//! project decides what adopting it means, and a later version already here is a door
//! lemonfiber cannot walk back through.

use crate::model::{CarryingReport, UnsupportedReport};
use crate::ports::docker::Image;

use super::version::{self, Standing};
use super::{image, Ours};

/// What no migration carries across, in any mode.
///
/// Stated rather than discovered, because these are the things an operator finds
/// missing weeks later and has no way to connect back to the day they migrated. A
/// migration that implied it took everything would be the more comfortable claim and
/// the one that costs them that afternoon.
const NEVER_CARRIED: [(&str, &str); 4] = [
    (
        "custom formats you wrote yourself",
        "they are scoring rules particular to your library, and lemonfiber sets its own from the \
         quality preset you choose",
    ),
    (
        "per-indexer tuning",
        "seed ratios, limits and priorities are set against indexers lemonfiber does not know you \
         hold accounts with",
    ),
    (
        "scripts hooked into an *arr's events",
        "a connect script runs a program on your machine, and carrying one across would run \
         somebody's program somewhere it was never pointed at",
    ),
    (
        "watch histories and play state",
        "they live in the media server's own database rather than in the wiring lemonfiber sets \
         up, and nothing here reads them",
    ),
];

/// What no migration carries across, whatever mode it runs in.
#[must_use]
pub fn not_carried() -> Vec<UnsupportedReport> {
    NEVER_CARRIED
        .iter()
        .map(|(what, because)| UnsupportedReport {
            what: (*what).to_owned(),
            because: (*because).to_owned(),
        })
        .collect()
}

/// What adopting each service lemonfiber recognises would come to.
#[must_use]
pub fn carrying(images: &[Image], project: &str, ours: &[Ours]) -> Vec<CarryingReport> {
    let mut found: Vec<CarryingReport> = ours
        .iter()
        .filter_map(|one| {
            let existing = image::standing_on(images, project, &one.image)?;
            Some(verdict(one, &existing))
        })
        .collect();
    found.sort_by(|one, two| one.service.cmp(&two.service));
    found
}

/// What adopting one service would come to, given the version already here.
fn verdict(ours: &Ours, existing: &str) -> CarryingReport {
    let (verdict, because, backup_first, refused) = match version::against(existing, &ours.tag) {
        Standing::Same => (
            "same",
            "the version already here is the one lemonfiber runs, so its database is opened \
             exactly as it stands"
                .to_owned(),
            false,
            false,
        ),
        Standing::Earlier => (
            "upgrade",
            format!(
                "lemonfiber runs {}, which upgrades this database on first start and is a step \
                 nothing walks back — so it is backed up before anything opens it",
                ours.tag
            ),
            true,
            false,
        ),
        Standing::Later => (
            "downgrade",
            format!(
                "this database has been through {existing}, and {} cannot open it afterwards — \
                 lemonfiber will not try, because the attempt is what damages it",
                ours.tag
            ),
            false,
            true,
        ),
        Standing::Untellable => (
            "cannot tell",
            format!(
                "neither {existing} nor {} reads as a version, so which came first cannot be \
                 told from the tags — it is backed up first rather than assumed safe",
                ours.tag
            ),
            true,
            false,
        ),
    };

    CarryingReport {
        service: ours.service.clone(),
        existing: existing.to_owned(),
        ours: ours.tag.clone(),
        verdict: verdict.to_owned(),
        because,
        backup_first,
        refused,
    }
}

#[cfg(test)]
mod tests;
