//! What this machine was last asked to run, and whether it was asked to stop.
//!
//! The record setup writes holds the operator's answer about starting on boot, and
//! an answer on its own cannot start anything: a machine that knows somebody wants
//! their stack back does not know *which* stack, and a machine that starts one
//! somebody deliberately stopped on Friday has ignored the clearest instruction it
//! was ever given. Both of those are answered here, by writing down what a lifecycle
//! command was actually asked to do.
//!
//! **Written where the operator's intent is known, which is not where the engine is
//! driven.** The guard on the data location stops the stack too, and it stops it
//! precisely because nobody chose to — so a rule that read every teardown as a
//! decision would leave a machine that unplugged a drive once and never brought its
//! stack back again. So only the two whole-stack actions are recorded, and the
//! service-by-service ones are not: stopping one container is not the operator
//! putting their stack down for the night.
//!
//! Best effort on the way out, like the conditions and the outbox beside it. A
//! record that could not be written costs the next boot its knowledge of which form
//! to bring back — which falls through to every form, the same answer naming no form
//! gives everywhere else — and that is a worse answer rather than a wrong claim, and
//! never worth failing a start over.

use crate::autostart::Returning;
use crate::model::LifecycleReport;
use crate::stack::compose::Action;

use crate::app::Ctx;

/// The record's file name, named once so the layout and the readers agree.
///
/// The same file [`crate::config::paths::Paths::autostart`] names. Setup writes the
/// answer into it before anything has ever been run, and this writes the rest of it
/// afterwards, which is why both spellings have to land on one file.
const RECORD: &str = "autostart.json";

/// What is to come back after a restart, as the last run left it.
#[must_use]
pub(crate) fn load(ctx: &Ctx) -> Returning {
    crate::app::record::beside(ctx, RECORD)
}

/// Write it where the next run — and the next boot — will read it.
pub(crate) fn save(ctx: &Ctx, returning: &Returning) {
    crate::app::record::keep_beside(ctx, RECORD, returning);
}

/// What one lifecycle action says about what this machine is for.
///
/// Two of the eight say anything at all. The rest change what is running without
/// saying anything about what should be: stopping one container to look at it,
/// fetching an image, or taking back a container an install put there and could not
/// prove, is not the operator putting their stack down for the night.
enum Said {
    /// These forms are what the operator wants running.
    Running,
    /// The stack is to stay down until they say otherwise.
    Stopped,
}

/// What this action amounts to, or nothing where it amounts to nothing.
const fn said(action: &Action) -> Option<Said> {
    match action {
        Action::Up => Some(Said::Running),
        Action::Down => Some(Said::Stopped),
        Action::Start(_)
        | Action::Stop(_)
        | Action::Remove(_)
        | Action::Restart(_)
        | Action::Pull
        | Action::Config => None,
    }
}

/// Write down what this lifecycle command was asked to do, where it did it.
///
/// Called from both ways of running one. A start can be waited on or streamed, and
/// the streamed one is what an operator at a terminal actually uses — so a record
/// written on only the waited-on path is a record that is right in the tests and
/// empty on every real machine.
pub(crate) fn noted(ctx: &Ctx, action: &Action, forms: &[String], report: &LifecycleReport) {
    if !carried(ctx, report) {
        return;
    }
    let Some(said) = said(action) else {
        return;
    };
    let mut returning = load(ctx);
    match said {
        Said::Running => returning.started(forms),
        Said::Stopped => returning.stopped(),
    }
    save(ctx, &returning);
}

/// Whether this run actually did the thing it was asked to do.
///
/// A rehearsal is excluded first and by itself, because it is the one case where the
/// report describes something that did not happen: a rehearsal reports the plan it
/// would have run, and a record written from it would say the stack was started by a
/// command whose whole promise is that it touched nothing.
fn carried(ctx: &Ctx, report: &LifecycleReport) -> bool {
    !ctx.dry_run && !report.rehearsed && report.status == Some(0)
}

#[cfg(test)]
mod tests;
