//! Answering a warning about something the operator chose on purpose.
//!
//! The other half of what can be done about a diagnosis, and the half that changes
//! the report rather than the stack. Some of what a check reports is not a fault: it
//! is a decision, and running torrents with nothing containing them is the one that
//! matters. Saying what it costs is right, once. Saying it on every run afterwards
//! teaches an operator that the tool repeats itself, after which they skim past the
//! findings that *are* faults.
//!
//! **Only a warning this stack is raising can be answered.** The core refuses an
//! accept naming anything else, and it is right to — recording an answer to something
//! nothing warned about would leave somebody believing they had settled a question
//! that goes on being put. So the warnings are asked for first and offered as a list
//! to take one of, which means this screen cannot send an accept that comes back
//! refused. That is the rule [`crate::acting::narrowing`] already picks a form and a
//! stuck item by: a thing already written down somewhere the screen can read it is
//! taken off a listing rather than typed from memory.
//!
//! **A failure is not offered, and neither is a pass.** A failure is not a choice, so
//! there is nothing about it to have weighed; a pass has nothing to answer. Both are
//! refused by the core, and a screen offering either would be offering a refusal.
//!
//! **The accepting run is not the listing run.** It looks again, over the whole suite,
//! because only something *that* run warns about can be answered — and a narrowed run
//! is one that may not have raised it. Where the warning has cleared in between, what
//! comes back is the core's own refusal, said where the operator is looking.

use lemonfiber_api::actions::Arguments;
use lemonfiber_core::app::Outcome;
use lemonfiber_core::doctor::Verdict;
use lemonfiber_core::model::DoctorReport;

use super::super::chooser::{Chooser, Listed};
use super::super::{Press, Stage, Wanted};
use super::Mending;

/// One warning this stack is raising, and what it is about.
pub(crate) struct Warning {
    /// The check raising it, as the finding names it — which is what an accept names.
    pub(crate) check: String,
    /// What it warns about, in the finding's own words.
    said: String,
}

impl Listed for Warning {
    fn name(&self) -> &str {
        &self.check
    }

    fn about(&self) -> &str {
        &self.said
    }
}

/// The warnings the diagnosis raised, or the diagnosis itself where it raised none.
///
/// A failure cannot be answered and a pass has nothing to answer, so neither is
/// offered: what an accept records is that a *choice* was weighed, and the core
/// refuses one naming anything this run is not warning about.
pub(super) fn raised(mending: &'static Mending, report: &DoctorReport, outcome: &Outcome) -> Stage {
    let mut raised = report
        .findings
        .iter()
        .filter_map(|finding| match &finding.verdict {
            Verdict::Warn(problem) => Some(Warning {
                check: finding.check.clone(),
                said: problem.summary.clone(),
            }),
            Verdict::Pass { .. }
            | Verdict::Fail(_)
            | Verdict::Unverified { .. }
            | Verdict::Skipped { .. } => None,
        });
    match raised.next() {
        Some(first) => Stage::Warned {
            mending,
            chooser: Chooser::over(first, raised.collect()),
        },
        None => super::read(outcome),
    }
}

/// Over the warnings: move, take one, or leave it.
pub(crate) fn warned(
    stage: &mut Stage,
    mending: &'static Mending,
    mut chooser: Chooser<Warning>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => {
            *stage = Stage::Answering {
                mending,
                warning: chooser.taken(),
            };
            return Wanted::Nothing;
        }
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Warned { mending, chooser };
    Wanted::Nothing
}

/// At the question: only an explicit yes answers the warning.
pub(crate) fn answering(
    stage: &mut Stage,
    mending: &'static Mending,
    warning: Warning,
    press: &Press,
) -> Wanted {
    if !matches!(*press, Press::Typed('y' | 'Y')) {
        return Wanted::Nothing;
    }
    let given = Arguments {
        check: Some(warning.check),
        ..Arguments::default()
    };
    super::sent(
        stage,
        mending,
        lemonfiber_api::actions::named(mending.action, given).map_err(|no| no.said()),
    )
}

#[cfg(test)]
mod tests;
