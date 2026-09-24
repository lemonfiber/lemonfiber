use super::{
    agreement, escalation, exhausted, mendable, offerable, offered, Outcome, Repair, Stance,
    ATTEMPTS,
};
use crate::condition::{Condition, Fault};
use crate::error::{Severity, State};

/// A raised condition for `check`, downstream of `cause` where one is given.
fn raised(check: &str, cause: Option<&str>) -> Condition {
    let mut fault = Fault::new(
        "service.stopped",
        Severity::Error,
        "it is not running",
        "nothing that needs it is working",
        "start it",
    );
    fault.caused_by = cause.map(str::to_owned);
    Condition::raised(check, &fault, "1000")
}

/// A repair answering `check` and nothing else.
fn repair(check: &str) -> Repair {
    Repair {
        check: check.to_owned(),
        does: "start it again".to_owned(),
        effects: Vec::new(),
        reversible: false,
    }
}

/// The names of what would be offered, which is what every case here turns on.
fn names(repairs: &[Repair], conditions: &[Condition]) -> Vec<String> {
    offered(repairs, &conditions.iter().collect::<Vec<_>>())
        .into_iter()
        .map(|repair| repair.check)
        .collect()
}

/// Undoing a repair undoes that repair — not the seed that ran before it, and not the
/// first-run wizard's writes. The journal is shared, and an operator asking to undo the
/// thing they just watched happen is not asking for everything lemonfiber ever wrote.
#[test]
fn undoing_a_repair_reverses_that_repair_and_nothing_else() {
    use super::{undoing, OPERATION};
    use crate::journal::{Change, Kind, Undo};

    let change = |operation: &str, at: &str, key: &str| Change {
        at: at.to_owned(),
        operation: operation.to_owned(),
        target: "sonarr".to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: Some("before".to_owned()),
            current: "after".to_owned(),
        },
    };

    let journal = vec![
        change("seed", "1000", "SEEDED"),
        change(OPERATION, "2000", "FIRST"),
        // The most recent repair, which set two values at once.
        change(OPERATION, "3000", "SECOND"),
        change(OPERATION, "3000", "THIRD"),
    ];

    let put_back = |key: &str| Undo {
        target: "sonarr".to_owned(),
        action: crate::journal::Action::Restore {
            key: key.to_owned(),
            value: Some("before".to_owned()),
            wrote: "after".to_owned(),
        },
    };

    // Both halves of the last repair, most recent first — and neither the seed nor the
    // repair before it.
    assert_eq!(
        undoing(&journal),
        vec![put_back("THIRD"), put_back("SECOND")]
    );

    // Nothing repaired yet is nothing to undo, rather than the whole journal.
    assert!(undoing(&[change("seed", "1000", "SEEDED")]).is_empty());
    assert!(undoing(&[]).is_empty());
}

/// The rule that keeps this feature from being something an operator has to defend
/// their machine against. A value they set is theirs; lemonfiber putting its own back
/// over it — however sure it is — is what makes people stop trusting a tool that
/// changes things.
///
/// Which is why the baseline is compared and not merely read. "lemonfiber wrote this
/// once" and "lemonfiber's value is what is there now" are different claims, and only
/// the second of them makes a field lemonfiber's to write again.
#[test]
fn a_repair_writes_over_what_lemonfiber_wrote_and_nothing_else() {
    use super::{may_write, Writing};
    use crate::baseline::{Origin, Record};

    let recorded = |origin| Record {
        value: "8080".to_owned(),
        at: "1000".to_owned(),
        origin,
    };

    // Its own work, still standing where it left it, put back.
    let ours = may_write(Some(&recorded(Origin::Written)), Some("8080"));
    assert_eq!(ours, Writing::Ours);
    assert!(ours.allowed());
    assert!(ours.refused().is_none());

    // Its own work, but the service no longer holds it. Reading the origin alone
    // would call this lemonfiber's to overwrite; what it actually is, is the edit
    // this rule exists to protect.
    let changed = may_write(Some(&recorded(Origin::Written)), Some("9090"));
    assert_eq!(changed, Writing::Changed);
    assert!(!changed.allowed());
    assert!(changed
        .refused()
        .is_some_and(|remedy| remedy.action.contains("since lemonfiber wrote it")));

    // Cleared rather than changed is still theirs — an emptied field is a decision
    // somebody took, not an absence to fill in.
    assert_eq!(
        may_write(Some(&recorded(Origin::Written)), None),
        Writing::Changed
    );

    // Theirs, adopted — left alone, and the operator told where to go instead. Read
    // before the comparison, so a value they have since moved again stays theirs
    // rather than becoming a change to argue about.
    let adopted = may_write(Some(&recorded(Origin::Adopted)), Some("7070"));
    assert_eq!(adopted, Writing::Adopted);
    assert!(!adopted.allowed());
    assert!(adopted
        .refused()
        .is_some_and(|remedy| remedy.action.contains("you set")));

    // Never written at all: a repair does not start writing somewhere lemonfiber has
    // never been, which is how a fix turns into a surprise.
    let theirs = may_write(None, Some("8080"));
    assert_eq!(theirs, Writing::TheirsAlone);
    assert!(!theirs.allowed());
    assert!(theirs.refused().is_some());

    // The one answer that is not a conclusion about a value: it is an instruction
    // about an area, and the sentence it carries has to say so rather than talk
    // about a change the operator may never have made.
    let declared = Writing::Unmanaged;
    assert!(!declared.allowed());
    let said = declared.refused().map(|remedy| remedy.action.clone());
    assert!(
        said.as_ref()
            .is_some_and(|said| said.contains("declared this unmanaged")),
        "{said:?}"
    );
}

/// Report-only unless this run said otherwise. A run that changed something because
/// nobody had said not to is the surprise that costs an operator their trust in
/// everything else the tool tells them.
#[test]
fn nothing_is_acted_on_unless_this_run_said_so() {
    assert_eq!(Stance::default(), Stance::ReportOnly);
    assert!(!Stance::ReportOnly.may_act());
    assert!(Stance::Ask.may_act());
    assert!(Stance::Unattended.may_act());
    assert!(Stance::Ask.asks());
    assert!(!Stance::Unattended.asks());
    assert!(!Stance::ReportOnly.asks());
}

/// "It ran" and "it worked" are different claims, and only the second one — proved by
/// asking the check again — settles anything.
#[test]
fn only_a_check_that_passes_afterwards_settles_a_fault() {
    assert!(Outcome::Fixed.settled());
    assert!(!Outcome::FixFailed.settled());
    assert!(!Outcome::Declined.settled());
    assert!(!Outcome::WouldOverwrite.settled());
    assert!(!Outcome::Stopped {
        leaving: "half of it".to_owned()
    }
    .settled());
}

/// The classification the operator sees is the one the problem already carried. A
/// second vocabulary invented here would be a second thing to keep in step.
#[test]
fn what_lemonfiber_can_mend_is_what_the_problem_already_said() {
    assert!(mendable(State::Remediable));
    assert!(!mendable(State::Actionable));
    assert!(!mendable(State::Guided));
    assert!(!mendable(State::Unknown));
    assert!(!mendable(State::Suppressed));
}

/// Three reasons not to offer one, and each is a different kind of no.
#[test]
fn a_repair_is_offered_while_it_is_wanted_and_might_still_work() {
    let mut condition = raised("service.sonarr", None);
    assert!(offerable(&condition));
    assert!(!exhausted(&condition));

    condition.declined = true;
    assert!(
        !offerable(&condition),
        "they said no and it has not been away"
    );
    condition.declined = false;

    condition.attempts = ATTEMPTS;
    assert!(!offerable(&condition));
    assert!(exhausted(&condition), "it has had its chances");
    condition.attempts = 0;

    condition.clear("2000");
    assert!(!offerable(&condition), "nothing to put right");
}

/// A fault that comes back deserves the attempt afresh: the count is about lemonfiber
/// being wrong, and a problem that genuinely returned is a new question.
#[test]
fn a_fault_that_comes_back_is_worth_trying_again() {
    let mut condition = raised("service.sonarr", None);
    condition.attempts = ATTEMPTS;
    condition.declined = true;
    condition.clear("2000");

    let fault = Fault::new(
        "service.stopped",
        Severity::Error,
        "it is not running",
        "nothing that needs it is working",
        "start it",
    );
    condition.raise(&fault, "3000");

    assert_eq!(condition.attempts, 0);
    assert!(!condition.declined);
    assert!(offerable(&condition));
}

/// Beyond what lemonfiber understands is exactly what a support bundle is for, and
/// somebody who has watched three repairs fail should not also have to work out the
/// flags — so the command is spelled out rather than described.
#[test]
fn a_repair_that_has_run_out_of_chances_hands_over_the_command() {
    let escalating = escalation(&raised("vpn.egress", None));
    assert!(escalating.action.contains("support bundle"));
    assert!(escalating.action.contains("vpn.egress"));
    assert_eq!(
        escalating.detail.as_deref(),
        Some("lemonfiber support --logs 500")
    );
}

/// One fault, one repair. A stack whose VPN is down raises a finding for every service
/// behind it, and six repairs for one fault is how a report stops being read.
#[test]
fn findings_sharing_a_cause_are_answered_once() {
    let conditions = vec![
        raised("vpn.up", None),
        raised("service.sonarr", Some("vpn.up")),
        raised("service.radarr", Some("vpn.up")),
    ];
    let repairs = vec![
        repair("vpn.up"),
        repair("service.sonarr"),
        repair("service.radarr"),
    ];

    assert_eq!(names(&repairs, &conditions), vec!["vpn.up".to_owned()]);
}

/// Downstream of something *not* being put right this pass, it is on its own — the
/// cause is only a reason to wait when waiting will actually answer it.
#[test]
fn a_dependent_is_offered_where_its_cause_is_not_being_answered() {
    let conditions = vec![raised("service.sonarr", Some("vpn.up"))];
    let repairs = vec![repair("service.sonarr")];

    assert_eq!(
        names(&repairs, &conditions),
        vec!["service.sonarr".to_owned()]
    );
}

/// A repair for something nothing has raised is not offered. The conditions are what
/// say a fault is standing right now; a repair list on its own says nothing.
#[test]
fn a_repair_with_nothing_wrong_behind_it_is_not_offered() {
    assert!(names(&[repair("service.sonarr")], &[]).is_empty());
    assert!(names(&[repair("service.sonarr")], &[raised("vpn.up", None)]).is_empty());
}

/// The shape a report carries, which is a contract the moment anything reads it: the
/// outcome is tagged by name, and the state that a stopped repair left behind rides
/// with it rather than being flattened into a word.
#[test]
fn an_outcome_reads_as_what_happened() {
    assert_eq!(
        serde_json::to_string(&Outcome::Fixed).ok(),
        Some(r#"{"outcome":"fixed"}"#.to_owned())
    );
    assert_eq!(
        serde_json::to_string(&Outcome::Stopped {
            leaving: "half of it".to_owned()
        })
        .ok(),
        Some(r#"{"outcome":"stopped","leaving":"half of it"}"#.to_owned())
    );
    assert_eq!(
        serde_json::to_string(&Outcome::FixFailed).ok(),
        Some(r#"{"outcome":"fix_failed"}"#.to_owned())
    );
    assert_eq!(
        serde_json::to_string(&Outcome::Declined).ok(),
        Some(r#"{"outcome":"declined"}"#.to_owned())
    );
    assert_eq!(
        serde_json::to_string(&Outcome::WouldOverwrite).ok(),
        Some(r#"{"outcome":"would_overwrite"}"#.to_owned())
    );
    assert_eq!(
        serde_json::to_string(&Stance::ReportOnly).ok(),
        Some(r#""report_only""#.to_owned())
    );
    assert_eq!(
        serde_json::to_string(&Stance::Ask).ok(),
        Some(r#""ask""#.to_owned())
    );
    assert_eq!(
        serde_json::to_string(&Stance::Unattended).ok(),
        Some(r#""unattended""#.to_owned())
    );
    assert!(serde_json::to_string(&repair("vpn.up"))
        .is_ok_and(|json| json.contains(r#""check":"vpn.up""#)));
}

/// An offer names itself by every word an operator reads before agreeing, so a
/// change to any of them is a different offer.
///
/// Each part on its own, because an agreement that ignored one of them would
/// let consent be spent on a repair whose statement had quietly changed — and
/// the one that matters most is what *else* changes if it goes ahead.
#[test]
fn an_offer_is_named_by_everything_that_was_read_before_agreeing() {
    let one = Repair {
        check: "vpn.port-forward-client".to_owned(),
        does: "move the download client onto the forwarded port".to_owned(),
        effects: vec!["transfers in flight pause briefly".to_owned()],
        reversible: true,
    };
    let name = agreement(std::slice::from_ref(&one));

    // The same offer, read twice, is the same offer.
    assert_eq!(name, agreement(std::slice::from_ref(&one)));

    let differing = [
        Repair {
            check: "vpn.killswitch".to_owned(),
            ..one.clone()
        },
        Repair {
            does: "restart the download client".to_owned(),
            ..one.clone()
        },
        Repair {
            effects: vec!["transfers in flight are cancelled".to_owned()],
            ..one.clone()
        },
        Repair {
            effects: Vec::new(),
            ..one.clone()
        },
        Repair {
            reversible: false,
            ..one.clone()
        },
    ];
    for other in differing {
        assert_ne!(name, agreement(std::slice::from_ref(&other)), "{other:?}");
    }

    // Order is part of it: the same repairs read in another order were read in
    // another order, and an offer is what was in front of somebody.
    let second = Repair {
        check: "vpn.killswitch".to_owned(),
        ..one.clone()
    };
    assert_ne!(
        agreement(&[one.clone(), second.clone()]),
        agreement(&[second, one])
    );
}

/// Two repairs whose words run together must not name the same offer as one
/// repair holding the joined text, which is what a checksum without ends does.
#[test]
fn two_words_do_not_run_together_into_a_third() {
    let split = Repair {
        check: "vpn".to_owned(),
        does: "port".to_owned(),
        effects: Vec::new(),
        reversible: true,
    };
    let joined = Repair {
        check: "vpnport".to_owned(),
        does: String::new(),
        effects: Vec::new(),
        reversible: true,
    };
    assert_ne!(agreement(&[split]), agreement(&[joined]));
}

/// An offer of nothing still names itself, because agreeing to nothing out of
/// it is a thing somebody can do.
#[test]
fn an_offer_of_nothing_still_names_itself() {
    assert!(!agreement(&[]).is_empty());
}
