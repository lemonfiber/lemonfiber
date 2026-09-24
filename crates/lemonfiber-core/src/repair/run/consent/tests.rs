use super::{Confirm as _, Consent, Report, Stance, STALE};
use crate::repair::{agreement, Repair};

fn repair(check: &str) -> Repair {
    Repair {
        check: check.to_owned(),
        does: "move the client onto the forwarded port".to_owned(),
        effects: vec!["transfers in flight pause briefly".to_owned()],
        reversible: true,
    }
}

fn given(offer: &str, repairs: &[&str]) -> Consent {
    Consent::Given {
        offer: offer.to_owned(),
        repairs: repairs.iter().map(|check| (*check).to_owned()).collect(),
    }
}

fn reporting(offered: Vec<Repair>) -> Report {
    Report {
        agreement: agreement(&offered),
        offered,
        ..Report::default()
    }
}

/// Each of the three says how the run may act, and no two say the same.
#[test]
fn each_consent_says_how_far_the_run_may_go() {
    assert_eq!(Consent::Offer.stance(), Stance::ReportOnly);
    assert_eq!(given("00000000", &[]).stance(), Stance::Ask);
    assert_eq!(Consent::Standing.stance(), Stance::Unattended);
}

/// Only a repair named in the consent is agreed to, and only a consent given
/// for an offer agrees to anything at all.
#[test]
fn only_what_was_named_is_agreed_to() {
    let consent = given("00000000", &["vpn.port-forward-client"]);

    assert!(consent.agreed(&repair("vpn.port-forward-client")));
    assert!(!consent.agreed(&repair("vpn.killswitch")));
    // Neither of the other two is ever asked, and both answer no if they are:
    // a run that only looks has agreed to nothing, and one told in advance was
    // not agreeing to a repair by name.
    assert!(!Consent::Offer.agreed(&repair("vpn.port-forward-client")));
    assert!(!Consent::Standing.agreed(&repair("vpn.port-forward-client")));
}

/// The offer that was read is compared with the offer that stands, so consent
/// cannot be spent on repairs the operator never saw.
#[test]
fn consent_given_for_one_offer_is_not_spent_on_another() {
    let offered = vec![repair("vpn.port-forward-client")];
    let name = agreement(&offered);

    assert!(given(&name, &["vpn.port-forward-client"]).stands(&offered));
    // The same check, and one more word about what else changes: a different
    // offer, because it is a different thing to have agreed to.
    let mut changed = offered.clone();
    if let Some(first) = changed.first_mut() {
        first.effects.push("and the client restarts".to_owned());
    }
    assert!(!given(&name, &["vpn.port-forward-client"]).stands(&changed));
    // The two that name no offer are never stale, because neither read one.
    assert!(Consent::Offer.stands(&changed));
    assert!(Consent::Standing.stands(&changed));
}

/// A report whose offer has moved on is refused, and says both names.
#[test]
fn a_report_that_moved_on_refuses_the_consent_that_named_the_old_one() {
    let stood = reporting(vec![repair("vpn.port-forward-client")]);
    let name = stood.agreement.clone();

    assert!(given(&name, &["vpn.port-forward-client"])
        .held(&stood)
        .is_ok());
    assert!(Consent::Offer.held(&stood).is_ok());
    assert!(Consent::Standing.held(&stood).is_ok());

    let refused = given("deadbeef", &["vpn.port-forward-client"])
        .held(&stood)
        .err()
        .map(|problem| (problem.code, problem.meaning.clone()));
    let (code, meaning) = refused.unwrap_or((STALE, String::new()));
    assert_eq!(code, STALE);
    // Both names, because "it changed" alone does not say whether a repair was
    // rewritten or a fault has cleared, and those ask for opposite things next.
    assert!(meaning.contains("deadbeef"), "{meaning}");
    assert!(meaning.contains(&name), "{meaning}");
}
