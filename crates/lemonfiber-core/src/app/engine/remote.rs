//! Refusing to act on a machine that has not got what the stack needs.
//!
//! With a remote context in force, Compose still reads its own files here and the
//! daemon still resolves every bind mount there. So a path that is perfectly
//! present on the laptop the operator is sitting at — the external drive, the
//! network share, the folder they picked during setup — is nothing at all on the
//! server, and Docker's answer to a bind mount whose host path is absent is to
//! create an empty directory and carry on. The stack comes up, the library is
//! empty, and nothing in the output says why.
//!
//! Unhandled, the error an operator eventually meets names a path that exists.
//! This is the pre-flight that says otherwise, in the shape the teardown guard next
//! door already has: asked before anything runs, refusing by naming the host and
//! the path rather than by stopping.
//!
//! Two checks, because they can be made at different moments. Whether the engine
//! can be driven at all needs nothing but the target and so guards every path that
//! builds an invocation; whether the location is there needs the other machine, and
//! is asked where a lifecycle command settles what it is about.
//!
//! The other machine is asked through its engine rather than through a shell on it.
//! That closes the endpoint a shell cannot reach — a daemon over TCP hands this no
//! filesystem and no credential — and it is the better question besides: the engine
//! is what resolves the mount, and a login on the same host can be looking at a
//! different filesystem than the daemon is.

use std::path::{Path, PathBuf};

use crate::app::Ctx;
use crate::error::{Diagnose, Problem, Remedy, Severity, State};
use crate::ports::docker::{Failure, Presence};

/// What is asked about, to find out whether the asking still works.
///
/// A name under the location itself, so the control question travels the same route
/// to the same daemon as the real one. Fixed rather than invented: if a machine
/// somehow has this, the check reports itself broken and the run goes on unverified,
/// which is the harmless direction to be wrong in.
const CANNOT_BE_THERE: &str = ".lemonfiber-is-this-check-working";

/// Whether this run may act on the engine it is pointed at at all.
///
/// Asked wherever an invocation is built, which is every path that can change
/// anything. An endpoint the Engine API cannot read must never become one Compose
/// writes to: that is the split this whole seam exists to close, and it would come
/// back the moment one half refused and the other carried on.
///
/// # Errors
///
/// Returns the [`Problem`] naming an endpoint nothing here can drive, or a Docker
/// context this machine does not have.
pub(crate) fn usable(ctx: &Ctx) -> Result<(), Box<Problem>> {
    match ctx.settings.docker.refusal() {
        None => Ok(()),
        Some(failure) => Err(Box::new(failure.problem())),
    }
}

/// Whether the machine being operated has the location the stack mounts.
///
/// `Ok(())` for a local run, which is every ordinary one, and for an operator who
/// has not chosen a location yet. A remote run this could not get an answer about
/// also proceeds — but it says so first, which is the whole difference between a
/// check that did not apply and one that quietly did not happen.
///
/// # Errors
///
/// Returns the [`Problem`] naming the host and the path where the location the
/// stack mounts is not on the machine the command would run against, the one
/// [`usable`] raises where the engine cannot be driven at all, and the engine's own
/// where the machine could not be reached to ask — a name that went nowhere and a
/// key that was refused are worth telling apart here rather than three commands
/// later.
pub(crate) async fn verified(ctx: &Ctx) -> Result<(), Box<Problem>> {
    usable(ctx)?;

    let target = &ctx.settings.docker;
    if !target.is_remote() {
        return Ok(());
    }
    let (Some(host), Some(root)) = (target.host(), ctx.settings.data_root.as_deref()) else {
        return Ok(());
    };

    match asked(ctx, root)
        .await
        .map_err(|err| Box::new(err.problem()))?
    {
        Verdict::There => Ok(()),
        Verdict::Absent => Err(Box::new(refusal(&host, root))),
        Verdict::Unreadable => {
            ctx.narrator.say(&unreadable(&host, root)).await;
            Ok(())
        }
        Verdict::Unmeasured => {
            ctx.narrator.say(&unmeasured(&host, root)).await;
            Ok(())
        }
    }
}

/// What asking the other machine about the location came to.
enum Verdict {
    /// The machine has it, and the asking was shown to work.
    There,
    /// The machine answered, and it has not.
    Absent,
    /// The machine was asked and said something that means neither.
    Unreadable,
    /// The machine said it was there, and then said the same of somewhere that is
    /// not — so the first answer is not evidence of anything.
    Unmeasured,
}

/// Ask the other machine about the location, and about whether it can still say no.
///
/// A yes is worth exactly what the instrument is worth, and this instrument rests on
/// an order the engine happens to check things in rather than on anything it
/// promises. If that order ever changes, every path starts reading as present and a
/// guard that has stopped guarding reports green forever — which is worse than no
/// guard, because it is a green somebody stops looking behind.
///
/// So a yes is only believed after somewhere that cannot be there comes back as not
/// there, from the same daemon, in the same breath. A no needs no such proof: an
/// answer that told one path from another is the proof.
async fn asked(ctx: &Ctx, root: &Path) -> Result<Verdict, Failure> {
    match ctx.locations.located(root).await? {
        Presence::Absent => Ok(Verdict::Absent),
        Presence::Unknown => Ok(Verdict::Unreadable),
        Presence::There => Ok(match ctx.locations.located(&control(root)).await? {
            Presence::Absent => Verdict::There,
            Presence::There | Presence::Unknown => Verdict::Unmeasured,
        }),
    }
}

/// Somewhere the answer must be no, if the asking works at all.
fn control(root: &Path) -> PathBuf {
    root.join(CANNOT_BE_THERE)
}

/// What to tell an operator whose stack has nowhere to live on the other machine.
///
/// Names both halves, because either one alone is the sentence that wastes an
/// afternoon: the path without the host reads as a path that is plainly there, and
/// the host without the path says nothing about what to make.
fn refusal(host: &str, path: &Path) -> Problem {
    let location = path.display();
    Problem::new(
        super::super::ABSENT_THERE,
        Severity::Error,
        format!("{location} is not on {host}"),
        format!(
            "A remote Docker context is in force, so this command would run against {host} — and \
             the stack mounts {location}, which is not there. It is on this machine, which is why \
             the path looks right. Nothing was started, because Docker would have made an empty \
             directory in its place and the services would have come up with nothing in them."
        ),
        Remedy::new(format!(
            "Make the location on {host}, or point lemonfiber at one that is there"
        ))
        .with_detail("lemonfiber config set DATA_ROOT <path on that machine>"),
    )
    .in_state(State::Guided)
}

/// What to say when the machine answered and the answer meant neither.
///
/// The operator's to act on, because the thing that might be wrong is theirs: a
/// daemon of a kind this has not met, a path it will not be asked about. It names
/// what was not established rather than what went wrong, because what went wrong is
/// not known — that is the condition.
fn unreadable(host: &str, path: &Path) -> String {
    format!(
        "could not check whether {} is on {host} — the engine there answered in a way this does \
         not recognise. Carrying on without that check: if the location is not on {host}, Docker \
         will make an empty directory in its place.",
        path.display()
    )
}

/// What to say when the check itself has stopped working.
///
/// A different sentence to a different reader. Nothing about the operator's machine
/// or their setting is wrong here — the check asked about somewhere that cannot
/// exist and was told it was there, so it can no longer tell the two apart, and no
/// amount of fixing a path would change that. Saying "could not verify the location"
/// would send them to look at the location.
fn unmeasured(host: &str, path: &Path) -> String {
    format!(
        "the check that proves {} is on {host} has stopped working — asked about somewhere that \
         cannot be there, {host} said it was, so its answer about the location means nothing. \
         Carrying on without the check. This is a fault in lemonfiber rather than in your setup, \
         and worth reporting.",
        path.display()
    )
}

#[cfg(test)]
mod tests;
