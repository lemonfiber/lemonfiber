//! Taking one of the things a listing came back with.
//!
//! Four of this screen's questions are about one thing rather than about everything,
//! and two of those four are about a thing that is already on a list. A form has a
//! name the stack chose and a stuck item has a title an \*arr is holding; asking an
//! operator to type either exactly would be a spelling test with a listing sitting
//! right there to be read off.
//!
//! So those two are asked twice. The question is put to the core as it stands, which
//! for a read given nothing is its own listing; what comes back is offered as a list
//! to move over rather than as an answer to read; and taking one puts the second
//! question. Nothing here decides what either question is — [`super::question`] holds
//! that, and the listing is [`super::question::Question::command`] given nothing
//! rather than a command written down beside it.
//!
//! **The second read need not be the first.** What is stuck is listed at
//! `/api/stuck` and each entry followed at `/api/trace`, which is the arrangement the
//! web surface already has: its stuck list names each item the way a trace is asked
//! for, so the count leads somewhere. This is that same pair of reads, walked with a
//! cursor instead of a link.
//!
//! **A listing with nothing in it is an answer, not a refusal.** A stack that
//! declares no forms and a queue with nothing stuck are both things an operator asked
//! about and got a true answer to, so what they get is the answer the command line
//! gives for the same read — rendered by the same renderer — rather than a sentence
//! this screen wrote about there being nothing to choose.

use lemonfiber_core::app::Outcome;
use lemonfiber_core::error::Problem;

use lemonfiber_core::app::Command;

use super::chooser::{Chooser, Listed};
use super::question::Question;
use super::reading::{complaint, lines_of, unexpected, Reading};
use super::{Press, Stage, Wanted};

/// One of the things a listing offered, and what asking about it comes to.
///
/// Its own type rather than the one an action's subjects are chosen with. That one
/// carries the forms a row names and whether it is marked, because several of those
/// rows are taken together; these are taken one at a time — a question is about one
/// thing or it is the listing — so a shape carrying a mark would be carrying a state
/// this list cannot be in.
pub(super) struct Subject {
    /// What it is called on the row.
    pub(super) name: String,
    /// What it is, in the line beside the name.
    pub(super) about: String,
    /// The command asking about this one comes to.
    pub(super) command: Command,
}

impl Listed for Subject {
    fn name(&self) -> &str {
        &self.name
    }

    fn about(&self) -> &str {
        &self.about
    }
}

/// What became of a question that was put to the core.
///
/// Either what it listed, to take one of, or what it answered — and a failure is an
/// answer like any other, said under the question that was asked rather than in the
/// box an action's own failures land in.
pub(super) fn asked(question: &'static Question, answer: Result<Outcome, Box<Problem>>) -> Stage {
    match answer {
        Ok(outcome) => answered(question, &outcome),
        Err(problem) => said(question, complaint(&problem)),
    }
}

/// What became of one of the things it listed.
///
/// An answer whatever shape it has. Put back through the listing instead, a trace of
/// one stuck item would be a shape that question cannot list — which would be
/// unexpected only to a screen that had forgotten it was on the second read.
pub(super) fn followed(
    question: &'static Question,
    answer: Result<Outcome, Box<Problem>>,
) -> Stage {
    said(
        question,
        match answer {
            Ok(outcome) => lines_of(&crate::render::shaped(&outcome)),
            Err(problem) => complaint(&problem),
        },
    )
}

/// Those lines, in a box titled by the question they answer.
fn said(question: &'static Question, lines: Vec<String>) -> Stage {
    Stage::Answered {
        question,
        reading: Reading::of(lines),
    }
}

/// What an answer to a question that narrows by picking came to.
///
/// Either the entries it listed, or the listing itself where it listed none.
fn answered(question: &'static Question, outcome: &Outcome) -> Stage {
    let Some((at, narrows)) = question.needs.picking() else {
        return read(question, outcome);
    };
    let Some(listed) = listed(outcome) else {
        return Stage::Came(Reading::of(unexpected()));
    };
    let mut choices = listed.into_iter().filter_map(|(names, name, about)| {
        super::question::asked_at(at, narrows, &names)
            .ok()
            .map(|command| Subject {
                name,
                about,
                command,
            })
    });
    match choices.next() {
        Some(first) => Stage::Narrowing {
            question,
            chooser: Chooser::over(first, choices.collect()),
        },
        None => read(question, outcome),
    }
}

/// The answer as the command line gives it, in a box to move through.
fn read(question: &'static Question, outcome: &Outcome) -> Stage {
    said(question, lines_of(&crate::render::shaped(outcome)))
}

/// Everything a listing offers: what taking each one names the second read, what it
/// is called on the row, and what it is in the line beside that.
///
/// The stack's own words for a form and the \*arr's own words for a stuck item. A
/// listing paraphrased here would be describing something other than what is running.
fn listed(outcome: &Outcome) -> Option<Vec<(String, String, String)>> {
    match outcome {
        Outcome::Forms(report) => Some(
            report
                .forms
                .iter()
                .map(|form| (form.id.clone(), form.name.clone(), form.description.clone()))
                .collect(),
        ),
        // The title is what a trace is asked by, which is why a stuck entry carries
        // one at all. Where it is held and how far it got are what tells two stuck
        // items apart on a row, so they are what the line beside the title says.
        Outcome::Stuck(report) => Some(
            report
                .items
                .iter()
                .map(|item| {
                    (
                        item.title.clone(),
                        item.title.clone(),
                        format!("{}, stuck at {}", item.service, item.stage.label()),
                    )
                })
                .collect(),
        ),
        _ => None,
    }
}

/// Over the entries a listing came back with: move, take one, or leave it.
pub(super) fn narrowing(
    stage: &mut Stage,
    question: &'static Question,
    mut chooser: Chooser<Subject>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => {
            *stage = Stage::Following(question);
            return Wanted::Carry(chooser.taken().command);
        }
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Narrowing { question, chooser };
    Wanted::Nothing
}
