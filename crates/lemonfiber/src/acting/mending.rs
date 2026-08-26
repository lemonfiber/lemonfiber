//! Putting right what a diagnosis found, and answering what it only warns about.
//!
//! Two writes behind one key, beside the reading they are both about. `doctor` is
//! already one of the questions [`super::question`] opens — every check, and what
//! each one found — and these two are what can be done about what that reading
//! reports. A finding lemonfiber can mend is put right; a warning about something the
//! operator chose deliberately is answered, so it stops leading.
//!
//! They are not on the list of errands, and the reason is the same one
//! [`super::quality`] gave for its three: the agreement does not mean the same thing
//! there.
//!
//! **An errand's yes is the whole of the agreement. A repair's yes is given to an
//! offer, for the repairs it names.** Every errand that carries one answers,
//! unconfirmed, with what it would do, and then a single yes carries the whole of it
//! out. A repair is the one action on any surface that shows the operator something
//! and then acts on *what they answered* — which is why
//! [`lemonfiber_api::actions::TAKES_CONSENT`] exists and why it holds this action
//! alone. A list whose rule is "the yes is the agreement" cannot take an action for
//! which the yes is a *selection* without the rule quietly becoming untrue for the
//! ones it was written for.
//!
//! **The agreement is bound to the offer it was read in.** The offer names itself,
//! and the consent sent back carries that name; the run that acts looks again and
//! refuses a name that is not what stands now. The command line gets that for nothing
//! by holding the question open in the process that asks it — this screen does not,
//! because the offer and the answer are two runs here as they are in a browser, so it
//! sends the name and lets the core compare. A screen that agreed to "whatever is
//! offered now" would be a surface that quietly re-scoped a repair between reading
//! and agreeing.
//!
//! **What each repair would do is read before the question, never after.** The
//! unconfirmed run *is* the offer, so the account the question sits under is not a
//! rehearsal of it — it is it. What else changes if a repair goes ahead, and whether
//! it can be taken back, are on the lines above the question rather than in what
//! comes back afterwards.
//!
//! **Only a warning this stack is raising can be answered.** The core refuses an
//! accept naming anything else, and it is right to: recording an answer to something
//! nothing warned about would leave an operator believing they had settled a question
//! that goes on being put. So the warnings are asked for first and offered as a list
//! to take one of, which means this screen cannot send an accept that comes back
//! refused — the rule [`super::narrowing`] already picks a form and a stuck item by.
//!
//! **`--undo` is not here.** It reads no offer, answers no warning and names no
//! subject at all: its yes is the whole of the agreement, which is the errands' rule
//! and not this list's. It sits on that list beside the other reversal, and the core
//! decides which repair was last and what putting it back takes.
//!
//! **The widening is not here either.** `--fix-disruptive` asks for the one thing
//! `--disruptive` asks for — [`lemonfiber_api::actions::TAKES_DISRUPTION`] carries a
//! single argument for all three actions, and the command line spells it twice only
//! because clap keys an argument by the field it sits on. Which half of a repair it
//! belongs to is the core's: the half that *acts*, an offer asked to include those
//! checks being refused rather than widened, because an offer is what somebody reads
//! before deciding and these checks prove themselves by disturbing. What is left of
//! it is a widening over checks that turn up no repair to offer. So this screen asks
//! for it where it is the thing being asked for — under the diagnosis, on the request
//! `--disruptive` is spelled on — and neither half of what is asked for here carries
//! it.

mod warning;

use lemonfiber_api::actions::{named, Arguments};
use lemonfiber_api::reads::{named as asked, Wanted as Asking, CHECKS};
use lemonfiber_core::app::repair::Report;
use lemonfiber_core::app::{Command, Outcome};
use lemonfiber_core::error::Problem;

use super::chooser::{Chooser, Listed};
use super::offer::MARKS;
use super::reading::{complaint, lines_of, moved, unexpected, Reading};
use super::{Press, Stage, Wanted};

pub(crate) use warning::Warning;
pub(super) use warning::{answering, warned};

/// The key that opens the two.
///
/// The letter the flag begins with: `doctor --fix` is what a shell asks for the same
/// thing, and `f` was free on a screen whose other letters had gone to the lifecycle
/// five and to the four lists that came before this one.
pub(crate) const KEY: char = 'f';

/// The word the footer puts beside that key.
pub(crate) const HINT: &str = "put right";

/// What is said beside a repair that cannot be taken back.
///
/// On the account rather than only on the row, because it is the one thing about a
/// repair that cannot be discovered afterwards: an operator who finds out that a
/// change was one-way after agreeing to it has been told nothing useful.
const ONE_WAY: &str = "this one cannot be put back afterwards";

/// What has to be read before the question can be put.
enum Reads {
    /// The offer itself. The unconfirmed run says what each repair would do and what
    /// else changes if it does, and changes nothing — so it is the account rather
    /// than a rehearsal of one.
    Offer,
    /// The warnings this stack is raising, which are the only things an accept can
    /// answer at all.
    Warnings,
}

/// One thing this screen can do about what a diagnosis found.
pub(crate) struct Mending {
    /// What it is called on the list, and on the box while it runs.
    pub(crate) name: &'static str,
    /// What it does, in one line.
    pub(crate) about: &'static str,
    /// The name every surface calls this action by.
    pub(crate) action: &'static str,
    /// How the question before it begins, what it is about completing it.
    pub(crate) asks: &'static str,
    /// What is said under that question, which is what agreeing comes to.
    pub(crate) costs: &'static str,
    /// What the box says while what has to be read first is with the core.
    pub(crate) waiting: &'static str,
    /// What has to be read before the question can be put.
    reads: Reads,
}

/// The one the list opens on.
///
/// Held apart from the rest for the reason the selected errand and the selected
/// question are: a list built from a slice that might have been empty carries a case
/// for there being nothing to choose, which is not a state this screen can be in.
static OPENS_ON: Mending = Mending {
    name: "what is wrong put right",
    about: "read what each repair would do, and carry out only the ones you agree to",
    action: "repair",
    asks: "Put right",
    costs: "only what you agreed to is carried out, and each is proved by asking the check again",
    waiting: "working out what could be put right",
    reads: Reads::Offer,
};

/// The one after it.
///
/// Second because it changes the report rather than the stack. Putting a fault right
/// is what somebody who asked what was wrong came here for; answering a warning is
/// what they do once they have decided the thing being warned about is what they
/// wanted — and that is the later decision of the two.
static AFTER: &[Mending] = &[Mending {
    name: "a warning you have already weighed",
    about: "accept a choice you made deliberately, so the check stops leading on it",
    action: "accept",
    asks: "Accept",
    costs: "the warning stops leading from now on, and nothing about this stack changes",
    waiting: "asking what this stack is warning about",
    reads: Reads::Warnings,
}];

impl Listed for Mending {
    fn name(&self) -> &str {
        self.name
    }

    fn about(&self) -> &str {
        self.about
    }
}

impl Mending {
    /// What is asked for before the question, in the words the surface it goes
    /// through gives for it.
    ///
    /// Two doors, because the two are different kinds of thing. An offer is the
    /// unconfirmed half of a write and goes through the table of actions; what this
    /// stack is warning about is a read and goes through the table of reads. Neither
    /// is assembled here, so this screen cannot ask for something no other surface
    /// can ask for.
    fn asking(&self) -> Result<Command, String> {
        match self.reads {
            Reads::Offer => named(self.action, Arguments::default()).map_err(|no| no.said()),
            Reads::Warnings => asked(CHECKS, Asking::default()).map_err(str::to_owned),
        }
    }
}

/// The offer as it stands on the screen: the name it gave itself, and what it holds.
///
/// One thing rather than two beside each other, because neither half means anything
/// without the other. The name is what the consent will be sent under, and the rows
/// are what it will be sent for — carried together so that no flow can put the second
/// through a stage that dropped the first.
pub(crate) struct Offering {
    /// The offer the repairs were read in, as it named itself.
    agreement: String,
    /// What it offered, the marked rows being what is agreed to.
    chooser: Chooser<Proposed>,
}

/// What was agreed to out of that offer, and the words it was agreed to in.
pub(crate) struct Agreed {
    /// The offer the repairs were read in, as it named itself.
    agreement: String,
    /// The checks whose repairs were agreed to, as that offer names them.
    pub(super) checks: Vec<String>,
    /// What each of them would do, and what else changes if it does.
    pub(super) account: Reading,
}

impl Offering {
    /// What it offered, to be drawn as the list it is.
    pub(super) const fn offered(&self) -> &Chooser<Proposed> {
        &self.chooser
    }
}

/// One repair the offer held, and whether it has been agreed to.
pub(crate) struct Proposed {
    /// The check whose finding it answers, which is what the consent names it by.
    check: String,
    /// What it would do, in the offer's own words.
    does: String,
    /// What else changes if it does.
    effects: Vec<String>,
    /// Whether carrying it out is recorded well enough to be put back.
    reversible: bool,
    /// Whether it has been marked to be agreed to.
    marked: bool,
}

impl Listed for Proposed {
    fn name(&self) -> &str {
        &self.check
    }

    fn about(&self) -> &str {
        &self.does
    }

    fn marked(&self) -> Option<bool> {
        Some(self.marked)
    }
}

/// The two, the one the list opens on apart from the rest.
pub(super) fn all() -> (&'static Mending, Vec<&'static Mending>) {
    (&OPENS_ON, AFTER.iter().collect())
}

/// Every one of them, in the order they are read.
#[cfg(test)]
pub(super) fn every() -> impl Iterator<Item = &'static Mending> {
    std::iter::once(&OPENS_ON).chain(AFTER)
}

/// Over the two: move, take one, or leave it.
pub(super) fn righting(
    stage: &mut Stage,
    mut chooser: Chooser<&'static Mending>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return taken(stage, chooser.taken()),
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Righting(chooser);
    Wanted::Nothing
}

/// Ask for what has to be read before the question can be put.
fn taken(stage: &mut Stage, mending: &'static Mending) -> Wanted {
    match mending.asking() {
        Ok(command) => {
            *stage = Stage::Looking(mending);
            Wanted::Carry(command)
        }
        Err(said) => {
            *stage = Stage::Came(Reading::of(vec![said]));
            Wanted::Nothing
        }
    }
}

/// While that is with the core: back out, or wait for it.
pub(super) fn looking(stage: &mut Stage, mending: &'static Mending, press: &Press) -> Wanted {
    if matches!(*press, Press::Abandon) {
        return Wanted::Nothing;
    }
    *stage = Stage::Looking(mending);
    Wanted::Nothing
}

/// What came back: the offer to answer, the warnings to answer one of, or a reading.
///
/// Held by what was asked for rather than by the shape of what arrived, so an answer
/// of the wrong shape is said to be one rather than quietly read as the other.
pub(super) fn looked(mending: &'static Mending, answer: Result<Outcome, Box<Problem>>) -> Stage {
    let outcome = match answer {
        Ok(outcome) => outcome,
        Err(problem) => return Stage::Came(Reading::of(complaint(&problem))),
    };
    match (&mending.reads, &outcome) {
        (Reads::Offer, Outcome::Repair(report)) => offered(mending, report, &outcome),
        (Reads::Warnings, Outcome::Doctor(report)) => warning::raised(mending, report, &outcome),
        _ => Stage::Came(Reading::of(unexpected())),
    }
}

/// The repairs the offer held, or the answer itself where it held none.
///
/// An offer with nothing in it is an answer rather than a refusal, which is the rule
/// a listing with nothing on it is already read by: a stack with nothing lemonfiber
/// can put right is a thing somebody asked about and got a true answer to, so what
/// they get is the answer the command line gives for the same run.
fn offered(mending: &'static Mending, report: &Report, outcome: &Outcome) -> Stage {
    let mut proposals = report.offered.iter().map(|repair| Proposed {
        check: repair.check.clone(),
        does: repair.does.clone(),
        effects: repair.effects.clone(),
        reversible: repair.reversible,
        marked: false,
    });
    match proposals.next() {
        Some(first) => Stage::Marking {
            mending,
            offering: Offering {
                agreement: report.agreement.clone(),
                chooser: Chooser::over(first, proposals.collect()),
            },
        },
        None => read(outcome),
    }
}

/// An answer as the command line gives it, in a box to move through.
fn read(outcome: &Outcome) -> Stage {
    Stage::Came(Reading::of(lines_of(&crate::render::shaped(outcome))))
}

/// Over the repairs offered: move, mark, take the marked, or leave it.
///
/// Marked the way the stack's own forms are marked, by the same key, because it is
/// the same movement: several rows chosen together out of one list. Where none is
/// marked, enter takes the row under the cursor — which is what the line under every
/// list on this screen says it does.
pub(super) fn marking(
    stage: &mut Stage,
    mending: &'static Mending,
    mut offering: Offering,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return agreeing(stage, mending, offering),
        Press::Back => offering.chooser.back(),
        Press::Forward => offering.chooser.forward(),
        Press::Typed(MARKS) => mark(&mut offering.chooser),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Marking { mending, offering };
    Wanted::Nothing
}

/// Mark the row under the cursor, or take the mark off it.
///
/// Each repair stands on its own: agreeing to one says nothing about any other, which
/// is why the consent travels as a list of checks rather than as a yes. So nothing
/// else on the list moves when one row is marked.
fn mark(chooser: &mut Chooser<Proposed>) {
    for (here, proposal) in chooser.each() {
        if here {
            proposal.marked = !proposal.marked;
        }
    }
}

/// Put the question over what was marked, under what each of them would do.
fn agreeing(stage: &mut Stage, mending: &'static Mending, offering: Offering) -> Wanted {
    let Offering { agreement, chooser } = offering;
    let marked = chooser.listed().any(|(_, proposal)| proposal.marked);
    let taken: Vec<Proposed> = if marked {
        chooser
            .all()
            .into_iter()
            .filter(|proposal| proposal.marked)
            .collect()
    } else {
        vec![chooser.taken()]
    };
    *stage = Stage::Consenting {
        mending,
        agreed: Agreed {
            agreement,
            account: Reading::of(account(&taken)),
            checks: taken.into_iter().map(|proposal| proposal.check).collect(),
        },
    };
    Wanted::Nothing
}

/// What the repairs agreed to would do, and what else changes if they do.
///
/// Every word of it above the question. An effect somebody reads after agreeing to it
/// is not one they agreed to, and whether a change can be taken back is the half of a
/// repair nothing afterwards can supply.
fn account(agreed: &[Proposed]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for proposal in agreed {
        lines.push(format!("{} — {}", proposal.check, proposal.does));
        lines.extend(proposal.effects.iter().map(|effect| format!("  {effect}")));
        if !proposal.reversible {
            lines.push(format!("  {ONE_WAY}"));
        }
    }
    lines
}

/// At the question: move through the account, agree to it, or leave it.
///
/// Only an explicit yes goes ahead, the way every other question on this screen is
/// read. What it sends names the offer the repairs were read in, so an answer cannot
/// be spent on an offer that has moved on since.
pub(super) fn consenting(
    stage: &mut Stage,
    mending: &'static Mending,
    mut agreed: Agreed,
    press: &Press,
) -> Wanted {
    if moved(&mut agreed.account, press) {
        *stage = Stage::Consenting { mending, agreed };
        return Wanted::Nothing;
    }
    if !matches!(*press, Press::Typed('y' | 'Y')) {
        return Wanted::Nothing;
    }
    let Agreed {
        agreement, checks, ..
    } = agreed;
    sent(
        stage,
        mending,
        consented(mending.action, &agreement, checks),
    )
}

/// The consent, as the table of actions spells it: the offer it was read in, and the
/// checks agreed to out of that offer.
///
/// Both, or neither means anything. An agreement that does not say which offer it
/// answered cannot be checked against the offer that stands, and an offer nobody
/// agreed to any of is consent that has lost its subject — which is what that table
/// refuses either of them alone for.
fn consented(action: &str, agreement: &str, agreed: Vec<String>) -> Result<Command, String> {
    let given = Arguments {
        confirm: true,
        offer: Some(agreement.to_owned()),
        agreed,
        ..Arguments::default()
    };
    named(action, given).map_err(|no| no.said())
}

/// Send what was agreed to, or say why it comes to no command.
///
/// One place for both, so a refusal reads the same whichever of the two produced it —
/// and it is the same box every other refused translation on this screen opens.
fn sent(stage: &mut Stage, mending: &'static Mending, command: Result<Command, String>) -> Wanted {
    match command {
        Ok(command) => {
            *stage = Stage::Putting(mending);
            Wanted::Carry(command)
        }
        Err(said) => {
            *stage = Stage::Came(Reading::of(vec![said]));
            Wanted::Nothing
        }
    }
}

/// While the putting-right is with the core: leaving is the only thing left to ask.
///
/// Nothing is drawn over the panels while it runs. A repair reaches the services —
/// a download client is restarted onto the port the provider granted — and the panel
/// that says what each service is doing is behind this box.
pub(super) fn putting(stage: &mut Stage, mending: &'static Mending, press: &Press) -> Wanted {
    *stage = Stage::Putting(mending);
    if super::leaving(press) {
        return Wanted::Leave;
    }
    Wanted::Nothing
}

#[cfg(test)]
pub(crate) mod tests {
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
        let (wanted, waiting) =
            pressed(Stage::Righting(Chooser::over(first, rest)), &Press::Accept);

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
}
