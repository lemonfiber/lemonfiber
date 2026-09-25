use super::Step;
use super::{Reason, Stopped};

#[test]
fn a_stop_carries_the_step_its_reason_belongs_to() {
    for reason in Reason::all() {
        assert_eq!(Stopped::plain(reason).step, reason.step(), "{reason:?}");
    }
}

#[test]
fn the_two_that_look_identical_are_told_apart() {
    // The single most-confused pair in the whole product: indexers that could not be
    // reached, and indexers that were reached and had nothing. Same absence, entirely
    // different problems, and only one of them is a fault.
    assert_ne!(Reason::IndexersFailed.said(), Reason::NothingMatched.said());
    assert!(Reason::IndexersFailed.is_a_fault());
    assert!(!Reason::NothingMatched.is_a_fault());
    assert!(!Reason::NoneMetThePreset.is_a_fault());
    assert_eq!(Reason::IndexersFailed.step(), Reason::NothingMatched.step());
}

#[test]
fn every_reason_names_a_step_says_what_happened_and_offers_a_way_out() {
    for reason in Reason::all() {
        assert!(!reason.said().is_empty(), "{reason:?}");
        assert!(!reason.remedy().is_empty(), "{reason:?}");
        // A remedy is an instruction, not a restatement of the problem.
        assert_ne!(reason.remedy(), reason.said(), "{reason:?}");
        assert!(Step::all().contains(&reason.step()), "{reason:?}");
    }
}

#[test]
fn a_stop_can_quote_what_the_services_were_saying() {
    let stopped = Stopped::quoting(
        Reason::ImportFailed,
        vec!["Sonarr: no files found are eligible for import".to_owned()],
    );
    assert_eq!(stopped.step, Step::Importing);
    assert_eq!(stopped.logs.len(), 1);
    assert_eq!(stopped.remedy, Reason::ImportFailed.remedy());
    assert!(Stopped::plain(Reason::ImportFailed).logs.is_empty());
}

#[test]
fn a_torrent_without_a_tunnel_stops_before_anything_is_grabbed() {
    // The one stop that exists to prevent an action rather than report one: a
    // tutorial is never worth a torrent outside the tunnel.
    assert_eq!(Reason::TunnelDown.step(), Step::Grabbing);
    assert!(Reason::TunnelDown.said().contains("nothing was grabbed"));
    assert!(Reason::TunnelDown.is_a_fault());
}
