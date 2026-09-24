use super::{
    account, all, answering, consented, consenting, every, looked, looking, marking, putting,
    righting, warned, Agreed, Mending, Proposed, Reads, KEY, ONE_WAY, OPENS_ON,
};
use crate::acting::chooser::Chooser;
use crate::acting::reading::Reading;
use crate::acting::{Press, Stage, Wanted};
use lemonfiber_api::actions::{OFFERED as WEB, TAKES_CONSENT, TAKES_DISRUPTION};
use lemonfiber_core::app::repair::{Confirm as _, Consent, Report};
use lemonfiber_core::app::{Command, Outcome};
use lemonfiber_core::doctor::{Category, Finding, Narrowing, Overall, Verdict};
use lemonfiber_core::error::{Code, Problem, Remedy, Severity};
use lemonfiber_core::model::DoctorReport;
use lemonfiber_core::repair::{agreement, Repair};

/// The one on the list with that action, which is how each test below reaches one.
pub(crate) fn doing(action: &str) -> &'static Mending {
    every()
        .find(|mending| mending.action == action)
        .unwrap_or(&OPENS_ON)
}

/// One repair an offer holds.
fn repair(check: &str, reversible: bool) -> Repair {
    Repair {
        check: check.to_owned(),
        does: format!("put {check} back the way it was declared"),
        effects: vec![format!("{check} restarts, so what it holds pauses briefly")],
        reversible,
    }
}

/// An offer over the repairs given, naming itself the way the core names it.
pub(crate) fn offering(offered: Vec<Repair>) -> Report {
    Report {
        agreement: agreement(&offered),
        offered,
        ..Report::default()
    }
}

/// A diagnosis warning about one thing and failing another, so that only the
/// first of the two can be answered.
pub(crate) fn a_diagnosis() -> DoctorReport {
    DoctorReport {
        overall: Overall::Degraded,
        findings: vec![
            Finding::in_category(
                Category::Vpn,
                "vpn.unprotected",
                "The download client is not behind the tunnel",
                Verdict::Warn(a_problem()),
            ),
            Finding::in_category(
                Category::Config,
                "config.wiring",
                "The services are wired to each other",
                Verdict::Fail(a_problem()),
            ),
        ],
    }
}

/// Something for a verdict to carry, the words being beside the point here.
fn a_problem() -> Problem {
    Problem::new(
        Code::new("VPN-9"),
        Severity::Warning,
        "Traffic leaves this machine outside the tunnel",
        "The download client's traffic was seen on this machine's own address.",
        Remedy::new("Put the client behind the gateway"),
    )
}

/// The screen having read an offer over the repairs given, and the name that
/// offer gave itself.
fn marking_over(offered: Vec<Repair>) -> (Stage, String) {
    let report = offering(offered);
    let named = report.agreement.clone();
    (looked(doing("repair"), Ok(Outcome::Repair(report))), named)
}

/// The screen having read what this stack is warning about.
pub(crate) fn warned_about() -> Stage {
    looked(doing("accept"), Ok(Outcome::Doctor(a_diagnosis())))
}

/// The screen having read an offer over two repairs, one of which cannot be put
/// back — which is the box the words this screen says about an offer are drawn
/// from.
pub(crate) fn an_offer() -> Stage {
    marking_over(vec![
        repair("vpn.port-forward-client", false),
        repair("config.wiring", true),
    ])
    .0
}

/// The same, with one repair marked and the question put over it.
pub(crate) fn an_agreement() -> Stage {
    let (_, marked) = pressed(an_offer(), &Press::Typed(' '));
    pressed(marked, &Press::Accept).1
}

/// The question put over one of the warnings this stack raises.
pub(crate) fn an_answer() -> Stage {
    pressed(warned_about(), &Press::Accept).1
}

/// What one press over a stage came to, and where it left the screen.
///
/// The stages this flow owns and no others: one it does not own comes back
/// exactly as it was, which is what lets a test feed one press straight into the
/// next without asking which flow the answer landed in.
pub(crate) fn pressed(stage: Stage, press: &Press) -> (Wanted, Stage) {
    let mut left = Stage::Idle;
    let wanted = match stage {
        Stage::Righting(chooser) => righting(&mut left, chooser, press),
        Stage::Looking(mending) => looking(&mut left, mending, press),
        Stage::Marking { mending, offering } => marking(&mut left, mending, offering, press),
        Stage::Consenting { mending, agreed } => consenting(&mut left, mending, agreed, press),
        Stage::Warned { mending, chooser } => warned(&mut left, mending, chooser, press),
        Stage::Answering { mending, warning } => answering(&mut left, mending, warning, press),
        Stage::Putting(mending) => putting(&mut left, mending, press),
        elsewhere => {
            left = elsewhere;
            Wanted::Nothing
        }
    };
    (wanted, left)
}

/// The checks a stage has agreed to, which is none anywhere but at the question.
fn agreed_in(stage: &Stage) -> Vec<String> {
    match stage {
        Stage::Consenting { agreed, .. } => agreed.checks.clone(),
        _ => Vec::new(),
    }
}
/// what this screen sends has to be something another surface already offers, or
/// the requirement it is being built for is defeated by the thing built for it.
#[test]
fn every_write_this_screen_offers_is_one_the_other_surfaces_offer() {
    let missing: Vec<&str> = every()
        .map(|mending| mending.action)
        .filter(|action| !WEB.contains(action))
        .collect();

    assert!(missing.is_empty(), "{missing:?}");
    assert!(every().all(|mending| !mending.about.is_empty()));
    assert!(every().all(|mending| !mending.costs.is_empty()));
    assert!(every().all(|mending| !mending.waiting.is_empty()));
}

/// The one action that shows the operator something and then acts on what they
/// answered is the one read off an offer. Which of the two that is is asked of
/// the table that says so, rather than decided a second time here.
#[test]
fn the_action_that_takes_a_consent_is_the_one_read_off_an_offer() {
    for mending in every() {
        let takes = TAKES_CONSENT.contains(&mending.action);
        let reads = matches!(mending.reads, Reads::Offer);
        assert_eq!(reads, takes, "{}", mending.name);
    }
}

/// The key this list opens on is not one the screen already answers, or the thing
/// it already did stops happening and nothing says so.
#[test]
fn the_key_that_opens_them_is_not_one_the_screen_already_answers() {
    for taken in [
        'q',
        'r',
        '?',
        crate::acting::question::KEY,
        crate::acting::errand::KEY,
        crate::acting::lasting::KEY,
        crate::acting::quality::KEY,
        crate::acting::surface::KEY,
    ] {
        assert_ne!(KEY, taken, "{taken:?} was already spoken for");
    }
    assert!(crate::acting::offer::OFFERED
        .iter()
        .all(|offer| offer.key != KEY));
}

/// The list opens on putting things right and holds both.
#[test]
fn the_list_opens_on_the_repair_and_holds_them_both() {
    let (first, rest) = all();

    assert_eq!(first.action, "repair");
    assert_eq!(rest.len() + 1, every().count());
}

/// The offer is asked for as the run that changes nothing: unconfirmed, naming no
/// offer and agreeing to nothing, which is the one shape the core reads as "say
/// what could be put right".
#[test]
fn the_offer_is_asked_for_as_the_run_that_changes_nothing() {
    assert_eq!(
        doing("repair").asking(),
        Ok(Command::Repair {
            consent: Consent::Offer,
            disruptive: false,
        })
    );
}

/// Neither half of a repair asked for here disturbs the stack. The widening is an
/// argument this screen reaches on the request it is about, and an offer that took
/// the tunnel away in order to be read would not be an offer.
#[test]
fn no_half_of_a_repair_asked_for_here_disturbs_the_stack() {
    assert!(TAKES_DISRUPTION.contains(&"repair"));

    assert_eq!(
        doing("repair").asking(),
        Ok(Command::Repair {
            consent: Consent::Offer,
            disruptive: false,
        })
    );
    assert_eq!(
        consented("repair", "00000000", vec!["vpn.killswitch".to_owned()]),
        Ok(Command::Repair {
            consent: Consent::Given {
                offer: "00000000".to_owned(),
                repairs: vec!["vpn.killswitch".to_owned()],
            },
            disruptive: false,
        })
    );
}

/// The warnings are asked for as the diagnosis every other surface reads, rather
/// than as a run of this screen's own.
#[test]
fn the_warnings_are_asked_for_as_the_diagnosis_every_surface_reads() {
    assert_eq!(
        doing("accept").asking(),
        Ok(Command::Doctor {
            narrowing: Narrowing::Suite,
            disruptive: false,
            accept: None,
        })
    );
}

/// The claim this slice turns on. What is sent names the offer the repairs were
/// read in, and the name travels from the very report the operator read — so the
/// core can refuse an answer given to an offer that has since moved on.
#[test]
fn the_consent_names_the_offer_the_repairs_were_read_in() {
    let (stage, named) = marking_over(vec![
        repair("vpn.port-forward-client", true),
        repair("config.wiring", false),
    ]);

    let (_, marked) = pressed(stage, &Press::Typed(' '));
    let (_, asked) = pressed(marked, &Press::Accept);
    let (wanted, running) = pressed(asked, &Press::Typed('y'));

    assert_eq!(
        wanted,
        Wanted::Carry(Command::Repair {
            consent: Consent::Given {
                offer: named,
                repairs: vec!["vpn.port-forward-client".to_owned()],
            },
            disruptive: false,
        })
    );
    assert!(matches!(running, Stage::Putting(_)));
}

/// The other half of that claim, put through the very comparison that enforces
/// it. Two offers differing by one word about what else changes are two offers,
/// and the consent this screen sends stands against the one it was read in and
/// falls against the one that moved on — which is what stops an answer being
/// spent on repairs nobody read.
#[test]
fn an_answer_read_in_one_offer_cannot_be_spent_on_another() {
    let mine = repair("vpn.port-forward-client", true);
    let mut moved_on = mine.clone();
    moved_on
        .effects
        .push("and every other client restarts too".to_owned());

    let (stage, named) = marking_over(vec![mine.clone()]);
    let (_, asked) = pressed(stage, &Press::Accept);
    let (wanted, _) = pressed(asked, &Press::Typed('y'));

    let sent = Consent::Given {
        offer: named,
        repairs: vec!["vpn.port-forward-client".to_owned()],
    };
    assert_eq!(
        wanted,
        Wanted::Carry(Command::Repair {
            consent: sent.clone(),
            disruptive: false,
        })
    );
    assert!(sent.stands(&[mine]), "against the offer it was read in");
    assert!(!sent.stands(&[moved_on]), "against one that has moved on");
}

/// Nothing marked agrees to the row under the cursor, which is what the line
/// under every list on this screen says enter does.
#[test]
fn nothing_marked_agrees_to_the_row_under_the_cursor() {
    let (stage, _) = marking_over(vec![
        repair("vpn.port-forward-client", true),
        repair("config.wiring", true),
    ]);

    let (_, moved_down) = pressed(stage, &Press::Forward);
    let (_, asked) = pressed(moved_down, &Press::Accept);

    assert_eq!(agreed_in(&asked), vec!["config.wiring".to_owned()]);
}

/// Each repair stands on its own: marking one says nothing about any other, which
/// is why the consent travels as a list of checks rather than as a bare yes.
#[test]
fn marking_one_repair_leaves_every_other_where_it_was() {
    let (stage, _) = marking_over(vec![
        repair("vpn.port-forward-client", true),
        repair("config.wiring", true),
    ]);

    let (_, first) = pressed(stage, &Press::Typed(' '));
    let (_, moved_down) = pressed(first, &Press::Forward);
    let (_, both) = pressed(moved_down, &Press::Typed(' '));
    let (_, asked) = pressed(both, &Press::Accept);

    assert_eq!(
        agreed_in(&asked),
        vec![
            "vpn.port-forward-client".to_owned(),
            "config.wiring".to_owned(),
        ]
    );
}

/// A mark taken off again is a repair not agreed to, or a row could be marked and
/// never unmarked.
#[test]
fn a_mark_taken_off_again_leaves_the_repair_out_of_the_agreement() {
    let (stage, _) = marking_over(vec![
        repair("vpn.port-forward-client", true),
        repair("config.wiring", true),
    ]);

    let (_, marked) = pressed(stage, &Press::Typed(' '));
    let (_, moved_down) = pressed(marked, &Press::Forward);
    let (_, second) = pressed(moved_down, &Press::Typed(' '));
    let (_, again) = pressed(second, &Press::Typed(' '));
    let (_, asked) = pressed(again, &Press::Accept);

    assert_eq!(
        agreed_in(&asked),
        vec!["vpn.port-forward-client".to_owned()]
    );
}

/// What each repair would do, what else changes if it does, and whether it can be
/// taken back are read before the question — never in what comes back afterwards.
#[test]
fn what_each_repair_would_do_is_read_before_the_question() {
    let said = account(&[
        Proposed {
            check: "vpn.port-forward-client".to_owned(),
            does: "move the client onto the forwarded port".to_owned(),
            effects: vec!["transfers in flight pause briefly".to_owned()],
            reversible: false,
            marked: true,
        },
        Proposed {
            check: "config.wiring".to_owned(),
            does: "point the service back at the client".to_owned(),
            effects: Vec::new(),
            reversible: true,
            marked: true,
        },
    ])
    .join("\n");

    assert!(
        said.contains("move the client onto the forwarded port"),
        "{said}"
    );
    assert!(said.contains("transfers in flight pause briefly"), "{said}");
    assert!(
        said.contains("point the service back at the client"),
        "{said}"
    );
    // Said of the one that cannot be taken back and of no other, or the sentence
    // means nothing on the line it is on.
    assert_eq!(said.matches(ONE_WAY).count(), 1, "{said}");
}

/// An offer holding nothing is an answer rather than a refusal, said in the words
/// the command line gives for the same run.
#[test]
fn an_offer_with_nothing_in_it_is_read_as_the_answer_it_is() {
    let stage = looked(doing("repair"), Ok(Outcome::Repair(offering(Vec::new()))));

    assert!(matches!(stage, Stage::Came(_)));
}

/// An answer of the wrong shape is said to be one, and a run that failed is said
/// in the words its failure came with — neither is read as the other.
#[test]
fn an_answer_of_the_wrong_shape_is_said_to_be_one() {
    let wrong = looked(doing("repair"), Ok(Outcome::Doctor(a_diagnosis())));
    let failed = looked(doing("accept"), Err(Box::new(a_problem())));

    assert!(matches!(wrong, Stage::Came(_)));
    assert!(matches!(failed, Stage::Came(_)));
}

/// Only an explicit yes agrees to the repairs marked, and everything else that is
/// not a move puts the box away.
#[test]
fn only_an_explicit_yes_carries_the_repairs_out() {
    let (stage, _) = marking_over(vec![repair("vpn.port-forward-client", true)]);
    let (_, asked) = pressed(stage, &Press::Accept);

    let (wanted, left) = pressed(asked, &Press::Typed('n'));

    assert_eq!(wanted, Wanted::Nothing);
    assert!(matches!(left, Stage::Idle));
    assert!(agreed_in(&left).is_empty());
}

/// Every list here moves and is left the way every other list on this screen is,
/// and leaving takes nothing with it.
#[test]
fn every_list_moves_and_is_left_without_sending_anything() {
    let (first, rest) = all();
    let opened = Stage::Righting(Chooser::over(first, rest));

    let (marking, _) = marking_over(vec![
        repair("vpn.port-forward-client", true),
        repair("config.wiring", true),
    ]);

    for stage in [opened, warned_about(), marking] {
        let (wanted, moved_down) = pressed(stage, &Press::Forward);
        assert_eq!(wanted, Wanted::Nothing);
        let (_, back) = pressed(moved_down, &Press::Back);
        let (_, typed) = pressed(back, &Press::Typed('z'));
        let (_, rubbed) = pressed(typed, &Press::Rubout);
        let (wanted, left) = pressed(rubbed, &Press::Abandon);

        assert_eq!(wanted, Wanted::Nothing);
        assert!(matches!(left, Stage::Idle));
    }
}

/// Taking one off the list asks for what has to be read before the question, and
/// backing out while that is with the core sends nothing.
#[test]
fn taking_one_asks_for_what_has_to_be_read_first() {
    let (first, rest) = all();
    let (wanted, waiting) = pressed(Stage::Righting(Chooser::over(first, rest)), &Press::Accept);

    assert_eq!(
        wanted,
        Wanted::Carry(Command::Repair {
            consent: Consent::Offer,
            disruptive: false,
        })
    );

    let (wanted, still) = pressed(waiting, &Press::Forward);
    assert_eq!(wanted, Wanted::Nothing);
    assert!(matches!(still, Stage::Looking(_)));

    let (wanted, left) = pressed(still, &Press::Abandon);
    assert_eq!(wanted, Wanted::Nothing);
    assert!(matches!(left, Stage::Idle));

    let (wanted, over) = pressed(left, &Press::Forward);
    assert_eq!(wanted, Wanted::Nothing);
    assert!(matches!(over, Stage::Idle));
}

/// The account under the question moves, and moving it is not agreeing to it.
#[test]
fn the_account_moves_without_agreeing_to_anything() {
    let many: Vec<Repair> = (0..9)
        .map(|at| repair(&format!("config.wiring-{at}"), true))
        .collect();
    let (stage, _) = marking_over(many);
    let (_, marked) = pressed(stage, &Press::Typed(' '));
    let (_, asked) = pressed(marked, &Press::Accept);

    let (wanted, still) = pressed(asked, &Press::Forward);

    assert_eq!(wanted, Wanted::Nothing);
    assert!(matches!(still, Stage::Consenting { .. }));
}

/// A consent the table of actions will not carry is said where the operator is
/// looking, rather than sent and refused somewhere they are not.
#[test]
fn a_consent_that_reaches_no_command_is_said_rather_than_sent() {
    let mut stage = Stage::Idle;

    let wanted = consenting(
        &mut stage,
        doing("accept"),
        Agreed {
            agreement: "00000000".to_owned(),
            checks: vec!["vpn.unprotected".to_owned()],
            account: Reading::of(vec!["what it would do".to_owned()]),
        },
        &Press::Typed('y'),
    );

    assert_eq!(wanted, Wanted::Nothing);
    assert!(matches!(stage, Stage::Came(_)));
}

/// A list this screen could not ask for at all is said rather than opened.
#[test]
fn a_list_that_reaches_no_command_is_said_rather_than_opened() {
    static UNTRANSLATABLE: Mending = Mending {
        name: "a write nothing answers",
        about: "for the refusal a translation that reaches no command produces",
        action: "not an action any surface offers",
        asks: "Do the impossible",
        costs: "nothing, because there is nothing to do",
        waiting: "waiting for what will never come",
        reads: Reads::Offer,
    };
    let mut stage = Stage::Idle;

    let wanted = righting(
        &mut stage,
        Chooser::over(&UNTRANSLATABLE, Vec::new()),
        &Press::Accept,
    );

    assert_eq!(wanted, Wanted::Nothing);
    assert!(matches!(stage, Stage::Came(_)));
}

/// While it runs, leaving is the only thing left to ask — and everything else
/// leaves it running.
#[test]
fn a_run_that_puts_things_right_is_left_rather_than_stopped() {
    let (wanted, still) = pressed(Stage::Putting(doing("repair")), &Press::Forward);

    assert_eq!(wanted, Wanted::Nothing);
    assert!(matches!(still, Stage::Putting(_)));

    let (wanted, _) = pressed(still, &Press::Typed('q'));

    assert_eq!(wanted, Wanted::Leave);
}
