//! The two refusals a rehearsal can give, and which of them an answer comes to.
//!
//! Apart from the verdict beside them because they are what an operator reads: a
//! sentence written once stays the same across fifty commands, and the two kinds of
//! refusal are kept apart so the temporary never reads as the permanent.

use crate::error::codes::rehearse::{CANNOT, NOT_YET};
use crate::error::{Amiss, Problem, Remedy, Severity, State};

use super::{Asked, Rehearsal};

/// What one of the four answers comes to, given what was asked.
///
/// Apart from the run that reaches it, because one of the four is an answer no
/// command carries today. `Untaught` is the escape hatch a command added tomorrow
/// gets: the match over every command is exhaustive, so whoever adds one has to
/// choose a verdict, and this is the one that says "not yet" out loud rather than
/// quietly rehearsing something that would act. Every command has since been taught,
/// which leaves the arm shipped and unreachable through `permitted` — and a rule
/// nothing can enter is a rule nobody has checked. Taking the answer rather than the
/// command is what lets it be handed one.
///
/// Public for that reason and only that reason. The arm is reachable nowhere
/// inside this crate, and a test in a `#[cfg(test)]` module would enter the copy
/// built for tests while the copy that ships stayed unentered — which is a rule
/// checked in a build nobody runs.
///
/// # Errors
///
/// Returns the [`Problem`] that refuses `--dry-run` where what was asked cannot be
/// rehearsed, or has not been taught to report what it would do.
pub fn verdict(asked: &Asked) -> Result<(), Box<Problem>> {
    match asked.rehearsal {
        Rehearsal::Reads | Rehearsal::Reports => Ok(()),
        Rehearsal::Cannot(why) => Err(Box::new(refused(asked, why))),
        Rehearsal::Untaught => Err(Box::new(not_taught_yet(asked.named))),
    }
}

/// The refusal a command gives when it cannot rehearse what was asked of it.
///
/// Said here rather than at each handler for the reason the verdict is decided here:
/// a sentence written once is a sentence that stays the same across fifty commands,
/// and an operator who has met it on one recognises it on the next.
#[must_use]
pub(super) fn refused(asked: &Asked, why: &'static str) -> Problem {
    Problem::new(
        CANNOT,
        Severity::Error,
        format!("`{}` cannot be rehearsed", asked.named),
        format!(
            "Nothing was done. {why}, so a rehearsal of this would be a report with \
             nothing in it that a rehearsal could have found out."
        ),
        Remedy::new(format!(
            "Run `lemonfiber {}` without `--dry-run` when you mean it",
            asked.named
        )),
    )
    .lies_in(Amiss::Asking)
    .in_state(State::Guided)
}

/// The refusal a command gives when it changes things and has not been taught to
/// report what it would change.
///
/// Separate from [`refused`] because the two are separate facts about the world, and
/// an operator can act on the difference: this one is a gap somebody is closing, and
/// the message says so rather than implying a limitation that is not there.
///
/// Takes the name rather than the whole of what was asked, which is what lets a test
/// in `tests/` reach it. It is unreachable through [`super::permitted`] — every command has
/// been taught — so the copy in the shipped build is a function no run enters, and a
/// function no run enters is counted against every covered line beside it. Reaching
/// it from outside the crate is what exercises the copy that ships.
#[must_use]
pub fn not_taught_yet(named: &str) -> Problem {
    Problem::new(
        NOT_YET,
        Severity::Error,
        format!("`{named}` does not rehearse yet"),
        "Nothing was done. This command changes things and has not yet been taught to \
         say what it would change, so it refuses the flag rather than accepting it and \
         going ahead — which is what it used to do."
            .to_owned(),
        Remedy::new(format!(
            "Run `lemonfiber {named}` without `--dry-run` when you mean it"
        )),
    )
    .lies_in(Amiss::Asking)
    .in_state(State::Guided)
}
