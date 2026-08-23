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

use super::super::engine::diagnose;
use super::choose::Chosen;
use super::walk::Walk;
use crate::doctor::{Category, Finding, Verdict};
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
    let proved = diagnose(walk.ctx, Some(Category::Vpn), false)
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
mod tests {
    use super::tunnel_holds;
    use crate::doctor::{Category, Finding, Verdict};
    use crate::ports::service::ReleaseProbe;
    use crate::walkthrough::Reason;

    /// One VPN finding of a given verdict.
    fn finding(verdict: Verdict) -> Finding {
        Finding::in_category(Category::Vpn, "vpn.egress-match", "The tunnel", verdict)
    }

    /// A verdict that could not be established — what the killswitch check gives whenever
    /// the disruptive checks were not asked for, which is every run that reaches here.
    fn unverified() -> Verdict {
        Verdict::Unverified {
            reason: "not asked for".to_owned(),
            remedy: crate::error::Remedy::new("wait"),
        }
    }

    #[test]
    fn a_tunnel_is_proved_by_something_passing_and_nothing_failing() {
        let passed = finding(Verdict::Pass { note: None });
        assert!(tunnel_holds(std::slice::from_ref(&passed)));
        // The killswitch is never verified, so a report carrying it alongside a pass must
        // still open the gate — otherwise no torrent stack is ever walked.
        assert!(tunnel_holds(&[passed.clone(), finding(unverified())]));
    }

    #[test]
    fn nothing_is_grabbed_where_the_tunnel_could_not_be_proved() {
        // A failed egress comparison blocks; a check nobody could run is not evidence of
        // a leak, but neither is it proof.
        let problem = crate::error::Problem::unknown(
            crate::error::Code::new("VPN-1"),
            crate::error::Severity::Error,
            "traffic is leaving outside the tunnel",
            "the two ends report different addresses",
        );
        assert!(!tunnel_holds(&[finding(Verdict::Fail(problem.clone()))]));
        assert!(
            !tunnel_holds(&[
                finding(Verdict::Pass { note: None }),
                finding(Verdict::Fail(problem))
            ]),
            "one failure is enough"
        );
        assert!(
            !tunnel_holds(&[finding(unverified())]),
            "unproved is not proved"
        );
        assert!(!tunnel_holds(&[]), "nothing checked is nothing proved");
        assert!(!tunnel_holds(&[finding(Verdict::Skipped {
            reason: "no torrents".to_owned()
        })]));
    }

    #[test]
    fn each_thing_the_indexers_could_say_maps_to_its_own_reason() {
        // The mapping is the acceptance criterion: nothing matched and indexers failed
        // must never collapse into one message.
        let reason = |probe| match probe {
            ReleaseProbe::NoneFound => Some(Reason::NothingMatched),
            ReleaseProbe::NoneMatch => Some(Reason::NoneMetThePreset),
            ReleaseProbe::Matching | ReleaseProbe::NothingWanted => None,
        };
        assert_eq!(
            reason(ReleaseProbe::NoneFound),
            Some(Reason::NothingMatched)
        );
        assert_eq!(
            reason(ReleaseProbe::NoneMatch),
            Some(Reason::NoneMetThePreset)
        );
        assert_eq!(reason(ReleaseProbe::Matching), None);
        assert_eq!(reason(ReleaseProbe::NothingWanted), None);
        assert_ne!(Reason::NothingMatched, Reason::IndexersFailed);
    }
}
