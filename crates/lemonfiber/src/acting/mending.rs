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
use lemonfiber_api::read::table::{named as asked, Wanted as Asking, CHECKS};
use lemonfiber_core::app::{Command, Outcome};
use lemonfiber_core::error::Problem;
use lemonfiber_core::repair::run::Report;

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
pub(crate) mod tests;
