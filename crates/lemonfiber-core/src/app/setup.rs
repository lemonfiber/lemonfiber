//! Driving the setup wizard to a working configuration.
//!
//! The wizard decides what to ask; every surface walks it one step at a time through
//! [`SetupAction`], and this holds what those steps share: the egress guard the VPN
//! step asks of, the progress file between steps, and the apply and recovery that
//! turn the answers into configuration.

mod answering;
mod guarding;
mod proving;

pub use answering::{setup, SetupAction};
pub(crate) use guarding::checking;
pub use guarding::{Asking, Guard};

use std::path::Path;

use crate::app::apply::{self, Applying};
use crate::config::paths::Paths;
use crate::config::store;
use crate::error::codes::setup::{
    ALREADY_SET_UP, DOES_NOT_APPLY, GUARD_UNANSWERED, NOTHING_TO_RECOVER, NOT_REVIEWED,
};
use crate::error::{Code, Diagnose as _, Problem, Remedy};
use crate::model::SetUp;
use crate::wizard::{
    described, offer_setup, Choice, Phase, Progress, Recovery, Rejected, Resolution, Status, Vpn,
    Wizard,
};

/// Every refusal answering, applying or recovering setup answers with, which a client
/// names by its code.
pub const REFUSALS: [Code; 5] = [
    DOES_NOT_APPLY,
    ALREADY_SET_UP,
    NOT_REVIEWED,
    NOTHING_TO_RECOVER,
    GUARD_UNANSWERED,
];

/// The service filling `network.egress-guard` in the stack `ctx` sets up, where one does,
/// or the service its adapter fronts: what setup's VPN step asks of it.
#[must_use]
pub fn guard(ctx: &crate::app::Ctx) -> Option<Guard> {
    let manifest = ctx.stack.manifest().ok()?;
    let register = crate::app::plugins::read(ctx).unwrap_or_default();
    let installed = register.installed();
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let fillers = crate::wiring::Fillers::of(
        &manifest,
        installed,
        &crate::app::targets::chosen_fillers(ctx),
        project.as_deref(),
    );
    let (filler, _) = fillers.filling(lemonfiber_manifest::container::EGRESS_GUARD)?;
    let guarding = fillers.fronted_by(filler).unwrap_or(filler);
    Guard::of(&manifest, installed, &guarding.id)
}

/// Take back a VPN answer that no longer gives `guard` what it needs, so its step is
/// asked again; why, where it was taken back.
///
/// # Errors
///
/// The refusal [`Guard::taken`] gives the answer recorded, with the answer taken back
/// and the walk standing on the VPN step.
pub(crate) fn guarded(wizard: &mut Wizard, guard: Option<&Guard>) -> Result<(), Box<Problem>> {
    let Some(Vpn::Carrying(given)) = &wizard.answers().vpn else {
        return Ok(());
    };
    checking(guard, given)
        .map(drop)
        .inspect_err(|_| wizard.reask_vpn())
}

/// Whether the machine `paths` lays out is set up, given the progress a previous run
/// saved there.
///
/// A run to pick up beats a machine that looks configured, and is asked first for that
/// reason: an apply that stopped part-way has written settings that the configured-yet
/// question reads as a finished install.
pub(crate) fn set_up(saved: Option<&Progress>, paths: &Paths) -> SetUp {
    if Status::of(saved).unfinished() {
        SetUp::PartWay
    } else if offer_setup(paths.env_file().exists()) {
        SetUp::None
    } else {
        SetUp::Done
    }
}

/// The setup progress saved at `path`, or nothing where none is there or it does
/// not read.
///
/// What a later run reads to tell where a previous one got to — which the wizard's
/// `Status` classifies, and which a resumed apply is restored from. Absence and an
/// unreadable or unparsable file are the same "no progress to resume from" answer,
/// not a fault: a fresh machine has none, and a torn one is better begun again.
#[must_use]
pub fn progress_at(path: &Path) -> Option<Progress> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Take a gathered setup through review and apply it, from the answers it holds.
///
/// The one way answers become configuration, whether they were gathered in this
/// process or over a sequence of requests, and however far a previous attempt got:
/// review is entered only from a complete set of answers, so an incomplete one is
/// refused here rather than judged by each caller.
///
/// A failed apply leaves the wizard restored to `applying` with every answer
/// intact — apply persists them before it writes. Rolling that one step back to
/// review is the wizard's single backward edge, and from review apply carries it
/// forward again. Apply is idempotent, so this either finishes what an interrupted
/// run started or leaves the same recoverable marker to try again.
///
/// # Errors
///
/// Returns a [`Problem`] where applying the recorded answers fails, leaving the
/// marker at `applying` for another attempt.
pub fn resume(wizard: &mut Wizard, applying: &Applying<'_>) -> Result<(), Box<Problem>> {
    wizard.transition(Phase::Reviewing);
    apply::apply(wizard, applying)?;
    clear_progress(applying.paths);
    Ok(())
}

/// What an apply would write, without writing any of it.
///
/// The same gate [`resume`] passes and the same refusal where it does not: review is
/// reached only from a complete set of answers, so a rehearsal is turned back exactly
/// where the run that writes is turned back rather than by a second judgement beside
/// it. Below that gate an apply is nothing but writes — the progress file, the journal,
/// the data directory, the stack, every setting, the appetite and the autostart answer
/// — so this stops at it, and the plan the wizard now holds is the whole of the report.
///
/// # Errors
///
/// Returns the [`Problem`] a real apply gives for answers that have not reached review.
pub(crate) fn would_apply(wizard: &mut Wizard) -> Result<(), Box<Problem>> {
    if wizard.transition(Phase::Reviewing) {
        return Ok(());
    }
    Err(Box::new(apply::not_reviewed()))
}

/// Take an interrupted apply the way it was chosen out of, and leave nothing of it
/// stranded.
///
/// The whole of the recovery rather than the deciding half of it: the reversal is
/// computed by [`Recovery`], carried out here, and followed by whatever the choice
/// asked for next — so a surface offers three words and does none of the work, and
/// three surfaces cannot come to disagree about what one of them means.
///
/// Starting over removes the journal as well as the answers, because a reversal
/// already carried out and still recorded is one a later recovery would try again.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render where a recorded change could
/// not be reversed, or where the apply that follows a resume or a roll back fails —
/// which leaves the marker at `applying` for another attempt, exactly as an apply
/// asked for directly does.
pub(crate) fn recovered(
    wizard: &mut Wizard,
    applying: &Applying<'_>,
    choice: Choice,
) -> Result<(), Box<Problem>> {
    let paths = applying.paths;
    let journal = crate::app::recover::journal_at(&paths.journal())
        .map_err(|failure| Box::new(failure.problem()))?;
    let env = paths.env_file();
    match Recovery::of(&journal).resolve(choice) {
        Resolution::Resume => resume(wizard, applying),
        Resolution::RollBack(undos) => {
            crate::app::recover::undo(applying.confined, &undos, &env, Vec::new())?;
            resume(wizard, applying)
        }
        Resolution::StartOver(undos) => {
            crate::app::recover::undo(applying.confined, &undos, &env, Vec::new())?;
            clear_progress(paths);
            let _ = std::fs::remove_file(paths.journal());
            Ok(())
        }
    }
}

/// What an apply that stopped part-way had already written, each said plainly.
///
/// The partial state a recovery is chosen about. Read from the change journal the
/// apply wrote as it went, so the choice is offered against what really happened
/// rather than against a guess — and said in the journal's own words, so the list
/// reads the same wherever the choice is put.
///
/// Empty where the apply stopped before it wrote anything, which is worth being
/// able to tell apart from having written something. A journal that cannot be read is
/// said as that, in the one line the list then holds, rather than as a list with
/// nothing in it.
#[must_use]
pub fn written_so_far(paths: &Paths) -> Vec<String> {
    match crate::app::recover::journal_at(&paths.journal()) {
        Ok(journal) => journal.changes().iter().map(described).collect(),
        Err(failure) => vec![failure.to_string()],
    }
}

/// Save where the wizard has reached, so quitting mid-setup resumes rather than
/// restarts.
///
/// Best-effort: a progress file that could not be written costs the resume, not
/// the run, so it is not raised. The operator can still finish here; only a crash
/// before they do would lose what was gathered, which is what this guards against.
fn save(wizard: &Wizard, paths: &Paths) {
    let text = serde_json::to_string(wizard.progress()).unwrap_or_default();
    let _ = store::write(&paths.setup_progress(), &text);
}

/// Remove the saved progress once setup has fully applied.
///
/// The progress file exists to make an interrupted setup resumable, and to do
/// that it holds every gathered answer — the indexer key and the Usenet password
/// among them — in the clear. A finished apply has nothing left to resume and has
/// already written those secrets to their real home in the `.env`, so this second
/// copy is not left lying on disk. Best-effort, and safe as one: apply persisted
/// the durable `applied` marker immediately before this, so a removal that does
/// not happen leaves a file a later run still reads as a finished setup, not a
/// lost one — and removing a file already gone is not a failure.
fn clear_progress(paths: &Paths) {
    let _ = std::fs::remove_file(paths.setup_progress());
}

/// The problem of an answer that does not apply where setup is running.
///
/// Setup only asks what applies here, so a rejection means a prompt offered a
/// choice it should not have; the rejected answer names which in the detail.
fn does_not_apply(rejected: Rejected) -> Problem {
    Problem::new(
        DOES_NOT_APPLY,
        "An answer does not apply on this platform",
        "Setup only offers what applies where it runs, so the answer was not recorded. Nothing has been applied.",
        Remedy::new("Answer with a choice this platform offers"),
    )
    .with_detail(format!("{rejected:?}"))
}

#[cfg(test)]
mod tests;
