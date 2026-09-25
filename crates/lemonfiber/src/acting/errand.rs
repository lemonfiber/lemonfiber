//! The rest of what this stack can be told to do, and what becomes of one.
//!
//! Every errand behind one key. The screen already answers `q`, `r`, `?`, the key
//! that opens the questions and five actions of its own, and there is no letter
//! left that anybody would guess — so this is the arrangement [`super::question`]
//! already made for the reads, made again for the writes that are not the lifecycle
//! five. The next errand goes on the list without costing anybody a letter to
//! learn, which is why this paragraph counts none of them.
//!
//! The key promises only that there is more. A wiring, a capture, a bundle, an
//! archive put back and a revert have no one word between them that is not vaguer
//! than the names on the list, and a key claiming something the list does not
//! hold is worse than one claiming nothing — so what each errand is is said on the
//! row, where there is room to say it.
//!
//! **Putting the last repair back is one of these, and putting a fault right is
//! not.** The two are one errand read in both directions, and the command line says
//! so — `--fix` and `--undo` are two errands rather than four settings, which is why
//! it declares them apart. But only one of them fits this list's rule. A repair is
//! offered, read, and then agreed to *in part*: the yes is a selection out of an
//! offer, which is the one thing no errand here does. An undo reads nothing and names
//! nothing — which repair was last, what reversing it takes and which of those need a
//! service to reach are the core's to decide — so its yes is the whole of the
//! agreement, exactly as the wiring's and the capture's are. It sits beside the other
//! reversal, the narrower of the two first, so nobody reaching for the one that puts
//! back a single repair lands on the one that puts back the whole configuration.
//!
//! Each errand is held by the name every surface calls the action by, and that name
//! goes through [`lemonfiber_api::actions::named`] exactly as the five on their own
//! keys do. What an errand may be given is that table's too: the arguments are the
//! ones the command carries, and one it has nowhere to put is refused there rather
//! than dropped here.
//!
//! **What it would do is said before the question, not after.** Three of the six
//! answer what they would come to without changing anything — the reverts a reset
//! would make, what a bundle would hold, what an archive would overwrite — so those
//! three are asked that first, and the question goes under the answer. An effect
//! somebody learns about afterwards is not something they agreed to, which is the
//! reading `doctor --fix` already puts its own offers under.
//!
//! **A name, never a path.** The one errand that has to be given something is the
//! restore, and what it takes is the name a backup was written under. It goes
//! through the same translation a browser's does, which carries it as
//! [`lemonfiber_core::app::restore::Kept::Named`] and never as a path — so the name
//! is resolved beneath the backups directory by the core, and one holding a path or
//! climbing out of that directory is refused by name rather than followed.
//!
//! **A bundle is asked what it is to hold.** How much of each service's log to take is
//! typed on a line of its own and what becomes of media filenames is taken off a list,
//! and both are said in the question above the yes — so what is agreed to is what is
//! written. The careful answers are still where an operator who presses enter twice
//! lands: an empty line is the ordinary window, and the list opens on filenames
//! replaced.
//!
//! **Nothing this screen sends can reveal a setting.** Which settings are shown as
//! they are is the one thing about a bundle not offered here, and it is an exception
//! rather than a gap: a way past the withholding list on this surface would be a
//! capability no other surface has, on the surface least likely to be sitting behind a
//! login. The guard beside this list holds it — every bundle this screen can send is
//! held to naming none — which is also what makes the agreement beside it safe to
//! carry.

use lemonfiber_api::actions::{named, Arguments};
use lemonfiber_core::app::{Command, Outcome};
use lemonfiber_core::bundle::run::LINES;

mod given;
mod listed;

use super::bundling;
use super::chooser::{Chooser, Listed};
use super::inviting;
use super::reading::{moved, Reading};
use super::service;
use super::{Press, Stage, Wanted};

pub(crate) use given::{Accepts, Given, Needs};
pub(super) use listed::all;
#[cfg(test)]
pub(super) use listed::every;

/// How many digits the line a log window is typed on will hold.
///
/// Nine, which is every number the argument can carry and no number it cannot. A line
/// that refuses the tenth digit is a line that can only ever hold an answer, which is
/// why nothing here has to write a sentence about a window that is not a number.
const MOST_DIGITS: usize = 9;

/// The key that opens the rest of the errands.
pub(crate) const KEY: char = 'm';

/// The word the footer puts beside that key.
pub(crate) const HINT: &str = "more";

/// What an errand sends once it has been agreed to.
#[derive(Clone, Copy)]
enum Going {
    /// As it stands. The question is the whole of the agreement, because the
    /// command has no half that reports and changes nothing.
    Once,
    /// The agreement itself, the run before it having said what would be lost.
    Agreed,
    /// The file, the run before it having said what would go in it.
    Written,
    /// The name the offer gave itself, which is the only yes this one has.
    ///
    /// Apart from [`Going::Agreed`] because it is a stronger thing rather than a
    /// differently spelled one. That yes is a flag, and a flag can be set by somebody
    /// who never read the run before it; this is the offer's own name, so saying it at
    /// all means the offer was read. The command carries no flag it could be given
    /// instead — see [`lemonfiber_api::actions::TAKES_AGREEMENT`], which leaves this
    /// action out on purpose.
    Answered,
}

/// One errand this stack can be sent on.
pub(crate) struct Errand {
    /// What it is called on the list, and on the box while it runs.
    pub(crate) name: &'static str,
    /// What it does, in one line.
    pub(crate) about: &'static str,
    /// The name every surface calls this action by.
    pub(crate) action: &'static str,
    /// How the question before it begins, what it was given completing it.
    pub(crate) asks: &'static str,
    /// What it has to be given first.
    pub(crate) needs: Needs,
    /// The further acceptance this errand's own account can call for, where it can
    /// call for one: what it fills in what is sent, and the words the question says it
    /// in.
    ///
    /// Two errands can. A restore onto a machine whose data root is not the one the
    /// archive was taken against is held until that move is accepted, and an update
    /// with something still coming down is held until interrupting it is. Both facts
    /// are the core's and arrive on the run in front of the question, so the words are
    /// here and what an operator answers is what that account called for.
    accepts: Option<(Accepts, &'static str)>,
    /// What it sends once it has been agreed to.
    going: Going,
}

impl Listed for Errand {
    fn name(&self) -> &str {
        self.name
    }

    fn about(&self) -> &str {
        self.about
    }
}

impl Errand {
    /// What this errand would do, or nothing where it has no half that only reports.
    ///
    /// The arguments as they stand: unconfirmed, and writing nothing. Those are the
    /// runs the command answers with what it would come to, having touched nothing.
    fn would(&self, given: &Given) -> Option<Result<Command, String>> {
        match self.going {
            Going::Once => None,
            Going::Agreed | Going::Written | Going::Answered => Some(self.reaching(given.asked())),
        }
    }

    /// What this errand sends once the operator has agreed to it.
    ///
    /// What it was given, and the careful defaults for everything else a bundle would
    /// otherwise be free to hold.
    ///
    /// The yes is carried as the command's own agreement on both of the two that take
    /// one. It was not, on the bundle: the yes was spent on writing the file and the
    /// field the command reads for consent went out false on every bundle this screen
    /// ever wrote. Nothing about what was produced was different, because this screen
    /// names no setting to show as it is — and that is the point. An agreement that
    /// arrives only when it happens to matter is an agreement nothing carries.
    fn sent(&self, given: &Given) -> Result<Command, String> {
        let mut asked = given.asked();
        match self.going {
            // Nothing added: the yes is the name the offer gave itself, and it was put
            // into what this was given the moment that offer came back.
            Going::Once | Going::Answered => (),
            Going::Agreed => asked.confirm = true,
            Going::Written => {
                asked.write = true;
                asked.confirm = true;
            }
        }
        self.reaching(asked)
    }

    /// Whether what the unconfirmed run reported calls for the further acceptance this
    /// errand can carry, and what to fill and say where it does.
    ///
    /// The archive's own account of itself says which data root it was taken against,
    /// and a difference there is the one thing a re-point is for; an update's account
    /// names what the download clients are still working on, and something in that
    /// list is the one thing the wait is for. Read off the answer rather than asked of
    /// the operator up front, for the reason the account is put in front of the
    /// question at all: an effect somebody agrees to before hearing of it is not one
    /// they agreed to.
    fn accepting(&self, outcome: &Outcome) -> Option<(Accepts, &'static str)> {
        let (fills, said) = self.accepts?;
        let called_for = match outcome {
            Outcome::Restore(restoration) => restoration.would.relocation.is_some(),
            Outcome::Update(report) => !report.in_flight.is_empty(),
            _ => false,
        };
        called_for.then_some((fills, said))
    }

    /// The name the offer this errand just read gave itself, where its yes is that
    /// name rather than a flag.
    ///
    /// Read off the answer rather than remembered, for the reason the re-point above
    /// it is: the name is built from what the offer said, and this screen is not the
    /// thing that knows what it said.
    fn answering(&self, outcome: &Outcome) -> Option<String> {
        match (self.going, outcome) {
            (Going::Answered, Outcome::Letting(offer)) => Some(offer.agreement.clone()),
            _ => None,
        }
    }

    /// The command an errand comes to, or why it comes to none — in the words the
    /// web surface gives for the same request.
    fn reaching(&self, given: Arguments) -> Result<Command, String> {
        named(self.action, given).map_err(|no| no.said())
    }
}

/// Over the errands: move, take one, or leave it.
pub(super) fn sending(
    stage: &mut Stage,
    mut chooser: Chooser<&'static Errand>,
    press: &Press,
    services: &[(String, String, String)],
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return taken(stage, chooser.taken(), services),
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Sending(chooser);
    Wanted::Nothing
}

/// Send the errand that was taken, or open what it has to be given first.
///
/// A capture with no services to choose between is sent as it stands, which is the
/// whole stack — the request this screen has always made of it. A screen that could
/// not reach the engine has nothing to narrow to, and a list of one row would be
/// offering a choice that is not one.
fn taken(
    stage: &mut Stage,
    errand: &'static Errand,
    services: &[(String, String, String)],
) -> Wanted {
    match errand.needs {
        // Four errands open a line and what they do with the word differs, which is
        // [`given`]'s answer rather than this one's: what is decided here is only
        // that there is a line.
        Needs::Archive(asks)
        | Needs::Bundling(asks)
        | Needs::Download(asks)
        | Needs::Named(asks)
        | Needs::Invitation(asks) => {
            *stage = Stage::Naming {
                errand,
                asks,
                typed: String::new(),
            };
            Wanted::Nothing
        }
        Needs::Service => match service::for_the_errand(errand, services) {
            Some(inside) => {
                *stage = inside;
                Wanted::Nothing
            }
            None => begun(stage, errand, service::nothing_to_choose()),
        },
        Needs::Nothing => begun(stage, errand, Given::nothing()),
    }
}

/// Over the line being typed: type, take back, go on, or leave it.
pub(super) fn naming(
    stage: &mut Stage,
    errand: &'static Errand,
    asks: &'static str,
    mut typed: String,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return given(stage, errand, typed),
        Press::Rubout => {
            typed.pop();
        }
        Press::Typed(character) => took(errand, &mut typed, character),
        Press::Back | Press::Forward => (),
    }
    *stage = Stage::Naming {
        errand,
        asks,
        typed,
    };
    Wanted::Nothing
}

/// Take the character where this line will have it.
///
/// A log window is a number, so its line takes digits and no more of them than a
/// number the argument can carry. A line that will only ever hold an answer is a line
/// nothing has to write a refusal about: what is turned away is the keystroke, not the
/// request, and every other line takes what it is given.
fn took(errand: &Errand, typed: &mut String, character: char) {
    match errand.needs {
        Needs::Bundling(_) if !character.is_ascii_digit() || typed.len() >= MOST_DIGITS => (),
        _ => typed.push(character),
    }
}

/// What the line an errand was typed on comes to.
///
/// The name of an archive is the whole of what a restore has to be given, so it goes
/// straight to the run that says what would be overwritten. A log window is the first
/// of a bundle's two answers, so it opens the second.
fn given(stage: &mut Stage, errand: &'static Errand, typed: String) -> Wanted {
    match errand.needs {
        // Nothing typed is the ordinary window, which is the one careful default this
        // line can be left at — and the figure is carried rather than left out, so the
        // question above the yes says the number the command was given.
        Needs::Bundling(_) => {
            *stage = bundling::over(errand, typed.parse().unwrap_or(LINES));
            Wanted::Nothing
        }
        // Same line, different argument: which one the word fills is the errand's
        // business rather than the line's.
        Needs::Named(_) => begun(stage, errand, Given::named(typed)),
        Needs::Download(_) => begun(stage, errand, Given::downloaded(typed)),
        // The first of an invitation's three answers, so it opens the second rather
        // than going on. The name is carried rather than folded into the arguments
        // here, because the sentence above the yes says all three together and there
        // is no way to say two of them and add the third.
        Needs::Invitation(_) => {
            *stage = inviting::over(errand, typed);
            Wanted::Nothing
        }
        _ => begun(stage, errand, Given::typed(typed)),
    }
}

/// Ask what the errand would do, or put the question where it has nothing to say
/// first.
pub(super) fn begun(stage: &mut Stage, errand: &'static Errand, given: Given) -> Wanted {
    match errand.would(&given) {
        Some(Ok(command)) => {
            *stage = Stage::Weighing { errand, given };
            Wanted::Carry(command)
        }
        Some(Err(said)) => {
            *stage = Stage::Came(Reading::of(vec![said]));
            Wanted::Nothing
        }
        None => {
            *stage = Stage::Agreeing {
                errand,
                given,
                would: None,
            };
            Wanted::Nothing
        }
    }
}

/// While what it would do is with the core: back out, or wait for it.
pub(super) fn weighing(
    stage: &mut Stage,
    errand: &'static Errand,
    given: Given,
    press: &Press,
) -> Wanted {
    if matches!(*press, Press::Abandon) {
        return Wanted::Nothing;
    }
    *stage = Stage::Weighing { errand, given };
    Wanted::Nothing
}

/// What the core said the errand would do, held for the operator to read and answer.
///
/// The account is also what decides the question. A restore whose archive names
/// another machine's data root is a re-point, and the yes under that listing is the
/// yes to the move — so the question names it rather than leaving an operator to
/// agree to a restore and be refused for the one thing the listing had just told them.
pub(super) fn weighed(
    errand: &'static Errand,
    given: Given,
    outcome: &Outcome,
    would: Vec<String>,
) -> Stage {
    let given = match errand.accepting(outcome) {
        Some((fills, said)) => given.accepted(fills, said),
        None => given,
    };
    let given = match errand.answering(outcome) {
        Some(offer) => given.answering(offer),
        None => given,
    };
    Stage::Agreeing {
        errand,
        given,
        would: Some(Reading::of(would)),
    }
}

/// At the question: move through what it would do, agree to it, or leave it.
///
/// Only an explicit yes goes ahead, the way the teardown's own question is read and
/// the way each repair is offered. Everything else that is not a move puts the box
/// away and changes nothing.
pub(super) fn agreeing(
    stage: &mut Stage,
    errand: &'static Errand,
    given: Given,
    mut would: Option<Reading>,
    press: &Press,
) -> Wanted {
    if let Some(reading) = would.as_mut() {
        if moved(reading, press) {
            *stage = Stage::Agreeing {
                errand,
                given,
                would,
            };
            return Wanted::Nothing;
        }
    }
    if !matches!(*press, Press::Typed('y' | 'Y')) {
        return Wanted::Nothing;
    }
    match errand.sent(&given) {
        Ok(command) => {
            *stage = Stage::Doing { errand, given };
            Wanted::Carry(command)
        }
        Err(said) => {
            *stage = Stage::Came(Reading::of(vec![said]));
            Wanted::Nothing
        }
    }
}

/// While the errand is with the core: leaving is the only thing left to ask.
pub(super) fn doing(
    stage: &mut Stage,
    errand: &'static Errand,
    given: Given,
    press: &Press,
) -> Wanted {
    *stage = Stage::Doing { errand, given };
    if super::leaving(press) {
        return Wanted::Leave;
    }
    Wanted::Nothing
}

#[cfg(test)]
pub(crate) mod tests;
