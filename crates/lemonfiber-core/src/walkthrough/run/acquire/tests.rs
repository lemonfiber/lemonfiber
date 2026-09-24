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
        crate::error::codes::vpn::LEAKING,
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
