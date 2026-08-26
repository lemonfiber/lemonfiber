//! What became of what the screen asked the core for.
//!
//! The other half of [`super`]. A press says what to go and do — which is
//! [`super::Acting::pressed`] and the flows it routes to — and every one of those
//! that reaches the core comes back here, at a moment no keypress chose. The two
//! are apart because they are answerable to different things: a press is answerable
//! to what the operator is looking at, and an answer is answerable to what the
//! screen was waiting for when it arrived.
//!
//! **An answer nobody is waiting for changes nothing.** A question backed out of
//! while it was with the core, an action left, a walk whose screen has moved on —
//! each of those leaves an answer with no stage expecting it, and the rule for all
//! of them is the same: put it nowhere rather than over whatever the operator went
//! on to do. Which is why every one of these reads what the screen is waiting for
//! before it reads the answer.

use lemonfiber_core::app::Outcome;
use lemonfiber_core::error::Problem;
use lemonfiber_core::walkthrough::Line as Step;

use super::reading::{complaint, lines_of, unexpected, Reading};
use super::{errand, lasting, mending, narrowing, offer, quality, Acting, Asked, Chooser, Stage};

impl Acting {
    /// What the stack answered when it was asked what there is to act on.
    ///
    /// One answer and two things that may have been waiting for it: an action, and
    /// the guard, which insists on a form the same way three of the five actions do.
    /// Which of them was waiting is what the screen recorded when it asked.
    pub(crate) fn told(&mut self, answer: Result<Outcome, Box<Problem>>) {
        let Some(asked) = self.asked.take() else {
            return;
        };
        let report = match answer {
            Ok(Outcome::Forms(report)) => report,
            Ok(_) => {
                self.stage = Stage::Came(Reading::of(unexpected()));
                return;
            }
            Err(problem) => {
                self.stage = Stage::Came(Reading::of(complaint(&problem)));
                return;
            }
        };
        self.stage = match asked {
            Asked::Action(offer) => match offer.given(&report) {
                Ok((selected, rest)) => Stage::Choosing {
                    offer,
                    chooser: Chooser::over(selected, rest),
                },
                Err(refused) => Stage::Came(Reading::of(vec![refused])),
            },
            Asked::Guard(lasting) => match offer::guarding(lasting.action, &report) {
                Ok((selected, rest)) => Stage::Picking {
                    lasting,
                    chooser: Chooser::over(selected, rest),
                },
                Err(refused) => Stage::Came(Reading::of(vec![refused])),
            },
        };
    }

    /// One of a walk's steps, as the core said it.
    ///
    /// Handed on rather than rendered here: what a step reads as on a terminal is
    /// [`crate::render::walkthrough`]'s, and it is the same rendering a shell is
    /// given for the same walk.
    pub(crate) fn stepped(&mut self, line: &Step) {
        lasting::stepped(&mut self.stage, line);
    }

    /// What the action came to, or what the question was answered with.
    ///
    /// Which of the two is read off what the screen is waiting for. An answer
    /// nobody is waiting for any more — a question backed out of while it was with
    /// the core — leaves the screen as it stands rather than opening a box over
    /// whatever the operator went on to do.
    pub(crate) fn came_to(&mut self, answer: Result<Outcome, Box<Problem>>) {
        self.stage = match std::mem::replace(&mut self.stage, Stage::Idle) {
            Stage::Running { .. } => Stage::Came(Reading::of(match answer {
                Ok(Outcome::Lifecycle(report)) => {
                    lines_of(&crate::render::stack::lifecycle(&report))
                }
                Ok(_) => unexpected(),
                Err(problem) => complaint(&problem),
            })),
            // What an errand would do is the answer the operator reads before
            // agreeing, so it lands on the question rather than closing over it. A
            // failure ends the errand there: there is nothing to agree to.
            Stage::Weighing { errand, given } => match answer {
                Ok(outcome) => {
                    errand::weighed(errand, given, lines_of(&crate::render::shaped(&outcome)))
                }
                Err(problem) => Stage::Came(Reading::of(complaint(&problem))),
            },
            // An errand and the widened diagnosis land the same way. Both were
            // agreed to rather than asked, and nothing further is offered under
            // either — the second is already the run that disturbs.
            // What has to be read before a repair or an accept can be agreed to: the
            // offer itself, or the warnings this stack is raising. Both are read
            // before the question rather than reported after it.
            Stage::Looking(mending) => mending::looked(mending, answer),
            Stage::Doing { .. } | Stage::Disturbing | Stage::Putting(_) => {
                Stage::Came(Reading::of(match answer {
                    Ok(outcome) => lines_of(&crate::render::shaped(&outcome)),
                    Err(problem) => complaint(&problem),
                }))
            }
            // What a walk came to goes under the steps that were watched on the way,
            // which is the order a shell shows them in. A guard has said nothing
            // until now, so what it came to is the whole of its box.
            Stage::Keeping { said, .. } => lasting::came_to(
                said,
                match answer {
                    Ok(outcome) => lines_of(&crate::render::shaped(&outcome)),
                    Err(problem) => complaint(&problem),
                },
            ),
            // What an upgrade would cost is read before it is agreed to, so it lands
            // on the question the way an errand's account does.
            Stage::Costing { change } => match answer {
                Ok(outcome) => quality::costed(change, lines_of(&crate::render::shaped(&outcome))),
                Err(problem) => Stage::Came(Reading::of(complaint(&problem))),
            },
            // A choice this host could only transcode in software comes back held
            // rather than recorded, and the caution it comes back with is the account
            // the question that follows sits under.
            Stage::Applying { change, chosen } => match answer {
                Ok(outcome) => quality::applied(
                    change,
                    chosen,
                    &outcome,
                    lines_of(&crate::render::shaped(&outcome)),
                ),
                Err(problem) => Stage::Came(Reading::of(complaint(&problem))),
            },
            // Rendered by the same renderer the command line reaches for the same
            // answer, so the two surfaces cannot come to say different things about
            // one stack. A question that narrows by picking is answered with a list
            // to take one of instead, which is the same answer read as a choice —
            // and what one of those comes to is an answer again, whichever shape.
            Stage::Waiting { question, typed } => narrowing::asked(question, &typed, answer),
            Stage::Following(question) => narrowing::followed(question, answer),
            waiting_for_nothing => waiting_for_nothing,
        };
    }
}
