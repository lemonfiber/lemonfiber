//! Asking for it — the one irreversible step, and the gate in front of it.
//!
//! Everything before this reads. This writes: it tells a service to take something on and
//! go and find it. So the tunnel is proved first, because a tutorial is never worth
//! putting a torrent outside it, and because "the walkthrough leaked my address on my
//! first day" is the one failure that cannot be apologised for afterwards.
//!
//! Whether the indexers then found anything is read back and told apart carefully: an
//! indexer that could not be reached and one that answered with nothing look identical
//! from here and are entirely different problems.

use super::choose::Chosen;
use super::walk::Walk;
use crate::app::engine::diagnose;
use crate::doctor::{Category, Finding, Narrowing, Verdict};
use crate::ports::service::{Added, Catalogue, QualityReleases, ReleaseProbe};
use crate::walkthrough::{Line, Reason, Step};

/// Ask for it, and report what the indexers had to say.
pub(super) async fn acquire(walk: &mut Walk<'_>, chosen: &Chosen<'_>) -> Result<Added, Reason> {
    tunnel_is_up(walk).await?;

    let arr = chosen.arr;
    walk.say(Line::saying(Step::Searching, chosen.service()));

    let plan = arr
        .service
        .add_plan(chosen.kind())
        .await
        .map_err(|_| Reason::NotGrabbed)?;
    let added = arr
        .service
        .add(chosen.kind(), &chosen.entry, &plan)
        .await
        .map_err(|_| Reason::NotGrabbed)?;

    // The add asked the service to go and look, so what the indexers carry can now be
    // read back. This is the one place the two identical-looking absences are told apart.
    match arr
        .service
        .probe_releases(chosen.kind().release_id_param())
        .await
    {
        Err(_) => Err(Reason::IndexersFailed),
        Ok(ReleaseProbe::NoneFound) => Err(Reason::NothingMatched),
        Ok(ReleaseProbe::NoneMatch) => Err(Reason::NoneMetThePreset),
        // Nothing wanted means the service considers the item satisfied, which after an
        // add of something missing means the search has not run yet — the wait that
        // follows is exactly the right place for that.
        Ok(ReleaseProbe::Matching | ReleaseProbe::NothingWanted) => {
            walk.say(Line::saying(
                Step::Grabbing,
                format!("{} will fetch it", chosen.service()),
            ));
            Ok(added)
        }
    }
}

/// Prove the tunnel before anything is asked for, where torrents are in play.
///
/// The gate is on torrents being *configured*, not on the release that will be picked
/// being a torrent: which protocol wins is the service's decision, made after this, and a
/// gate that waited to find out would be a gate that opened first.
async fn tunnel_is_up(walk: &mut Walk<'_>) -> Result<(), Reason> {
    if !walk.ctx.settings.protocols.torrent {
        return Ok(());
    }
    let proved = diagnose(walk.ctx, &Narrowing::Category(Category::Vpn), false)
        .await
        .is_ok_and(|report| tunnel_holds(&report.findings));
    if proved {
        return Ok(());
    }
    Err(Reason::TunnelDown)
}

/// Whether the VPN findings amount to a tunnel that is proved up.
///
/// Not the category's own overall verdict, deliberately. The killswitch test drops the
/// tunnel to prove it, so it only runs where the operator asked for the disruptive checks
/// and this diagnosis does not ask — which leaves that finding undetermined every time,
/// dragging the category with it. Gating on the category would refuse every torrent stack
/// for ever, which is not a safety property but a permanently closed door.
///
/// What is asked instead is the honest question: did anything about the tunnel fail, and
/// did anything about it actually pass? A failed egress comparison blocks; a check nobody
/// could run is not evidence of a leak, but neither is it proof, so something has to have
/// passed before anything is grabbed.
fn tunnel_holds(findings: &[Finding]) -> bool {
    let mut proved = false;
    for finding in findings {
        match finding.verdict {
            Verdict::Fail(_) => return false,
            Verdict::Pass { .. } => proved = true,
            Verdict::Warn(_) | Verdict::Unverified { .. } | Verdict::Skipped { .. } => {}
        }
    }
    proved
}

#[cfg(test)]
mod tests;
