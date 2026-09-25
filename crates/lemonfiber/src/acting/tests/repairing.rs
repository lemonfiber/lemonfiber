//! Offering a repair, agreeing to it, and putting it back.

use super::*;

/// The claim this slice makes, end to end. Asking what is wrong was reachable and
/// putting it right was not: the offer is asked for, every word of it is read,
/// the repairs agreed to are marked one at a time, and only then does a yes send
/// the consent — naming the offer those repairs were read in.
#[test]
fn a_repair_is_offered_read_marked_and_only_then_agreed_to() {
    let offered = vec![
        a_repair("vpn.port-forward-client", false),
        a_repair("config.wiring", true),
    ];
    let report = an_offer(offered);
    let named = report.agreement.clone();

    let (mut acting, wanted) = putting("repair");
    assert_eq!(
        wanted,
        Wanted::Carry(Command::Repair {
            consent: Consent::Offer,
            disruptive: false,
        })
    );

    acting.came_to(Ok(Outcome::Repair(report)));
    let said = showing(&acting);
    assert!(said.contains("[ ] vpn.port-forward-client"), "{said}");
    assert!(said.contains("[ ] config.wiring"), "{said}");

    acting.pressed(&Press::Typed(' '));
    acting.pressed(&Press::Accept);
    let asked = showing(&acting);
    assert!(asked.contains("pauses briefly"), "{asked}");
    assert!(asked.contains("cannot be put back"), "{asked}");
    assert!(
        asked.contains("Put right vpn.port-forward-client?"),
        "{asked}"
    );

    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Repair {
            consent: Consent::Given {
                offer: named,
                repairs: vec!["vpn.port-forward-client".to_owned()],
            },
            disruptive: false,
        })
    );
}

/// The half of that claim a green run does not prove on its own: what is sent
/// names *this* offer, so an answer read in one cannot be spent on another. Two
/// offers differing by one word about what else changes are two offers, and the
/// core compares the name against a fresh look before it carries anything out.
#[test]
fn a_repair_agreed_to_in_one_offer_cannot_be_spent_on_a_later_one() {
    let mine = a_repair("vpn.port-forward-client", true);
    let mut moved_on = mine.clone();
    moved_on
        .effects
        .push("and every other client restarts too".to_owned());
    let later = an_offer(vec![moved_on]).agreement;

    let (mut acting, _) = putting("repair");
    acting.came_to(Ok(Outcome::Repair(an_offer(vec![mine]))));
    acting.pressed(&Press::Typed(' '));
    acting.pressed(&Press::Accept);

    let sent = acting.pressed(&Press::Typed('y'));

    let read_in = an_offer(vec![a_repair("vpn.port-forward-client", true)]).agreement;
    assert_ne!(read_in, later);
    assert_eq!(
        sent,
        Wanted::Carry(Command::Repair {
            consent: Consent::Given {
                offer: read_in,
                repairs: vec!["vpn.port-forward-client".to_owned()],
            },
            disruptive: false,
        })
    );
}

/// While the offer is with the core the box says what it is waiting for and
/// nothing else can be asked; while the repairs are being carried out nothing is
/// drawn over the panels and leaving is the only thing left to ask — the run goes
/// on, because the process that claimed the stack is the one carrying it out.
#[test]
fn a_repair_is_waited_for_and_then_left_running() {
    let (mut acting, _) = putting("repair");

    assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
    let waiting = showing(&acting);
    assert!(
        waiting.contains("working out what could be put right"),
        "{waiting}"
    );

    acting.came_to(Ok(Outcome::Repair(an_offer(vec![a_repair(
        "vpn.port-forward-client",
        true,
    )]))));
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Typed('y'));

    assert!(showing(&acting).is_empty());
    let footing = footing(&acting);
    assert!(footing.contains("what is wrong put right"), "{footing}");
    assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);

    assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);
}

/// A warning is answered off the very run that raised it. Only something a run
/// warns about can be accepted, so the warnings are asked for first and offered
/// as a list — which means this screen cannot send an accept that comes back
/// refused.
#[test]
fn a_warning_is_answered_off_the_run_that_raised_it() {
    let (mut acting, wanted) = putting("accept");
    assert_eq!(
        wanted,
        Wanted::Carry(Command::Doctor {
            narrowing: Narrowing::Suite,
            disruptive: false,
            accept: None,
        })
    );

    acting.came_to(Ok(Outcome::Doctor(a_warning())));
    let said = showing(&acting);
    assert!(said.contains("> vpn.unprotected"), "{said}");
    // The failure in the same report is not offered: a failure is not a choice,
    // and the core refuses an accept naming one.
    assert!(!said.contains("config.wiring"), "{said}");

    acting.pressed(&Press::Accept);
    let asked = showing(&acting);
    assert!(asked.contains("Accept vpn.unprotected?"), "{asked}");

    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Doctor {
            narrowing: Narrowing::Suite,
            disruptive: false,
            accept: Some("vpn.unprotected".to_owned()),
        })
    );
}

/// Putting the last repair back is an errand rather than one of those two. It
/// reads no offer and names no subject: the yes is the whole of the agreement,
/// which is that list's rule and not this one's.
#[test]
fn putting_the_last_repair_back_is_asked_for_as_an_errand() {
    let (mut acting, wanted) = sending("undo");

    assert_eq!(wanted, Wanted::Nothing);
    let asked = showing(&acting);
    assert!(
        asked.contains("Put back what the last repair changed?"),
        "{asked}"
    );
    assert!(!asked.contains("up and down move"), "{asked}");

    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Undo { run: None })
    );
}

/// One repair the offer holds.
fn a_repair(check: &str, reversible: bool) -> Repair {
    Repair {
        check: check.to_owned(),
        does: format!("put {check} back the way it was declared"),
        effects: vec![format!("{check} restarts, so what it holds pauses briefly")],
        reversible,
    }
}

/// An offer over those repairs, naming itself the way the core names it.
fn an_offer(offered: Vec<Repair>) -> RepairReport {
    RepairReport {
        agreement: agreement(&offered),
        offered,
        ..RepairReport::default()
    }
}

/// A diagnosis warning about one thing and failing another, so that only the
/// first of the two can be answered.
fn a_warning() -> DoctorReport {
    DoctorReport {
        overall: Overall::Degraded,
        findings: vec![
            Finding::in_category(
                Category::Vpn,
                "vpn.unprotected",
                "The download client is not behind the tunnel",
                Verdict::Warn(a_failure()),
            ),
            Finding::in_category(
                Category::Config,
                "config.wiring",
                "The services are wired to each other",
                Verdict::Fail(a_failure()),
            ),
        ],
    }
}
