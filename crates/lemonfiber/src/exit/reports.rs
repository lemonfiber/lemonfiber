//! What each report's own answer comes to, as an exit code.
//!
//! Apart from [`super`] because they are two questions. That file names every code
//! this binary can return and decides which a *problem* or an *outcome* deserves;
//! this decides what one report says about itself — whether a run that carried
//! records across left any behind, whether an install that reported was an install
//! that held.
//!
//! Every one of them is a rule over a report's own fields rather than over its type,
//! which is the whole reason they are functions and not a match arm each: a report
//! that arrived is not the same as a run that worked, and the difference is always in
//! what the report says rather than in which report it is.

use std::process::ExitCode;

use lemonfiber_core::app::repair::Report as RepairReport;
use lemonfiber_core::model::{
    AdoptReport, LifecycleReport, ResetReport, Revoked, Triggered, UpgradeReport, WizardReport,
};
use lemonfiber_core::wizard::Phase;

use super::{FAILURE, VALIDATION};

/// What a run that carried an operator's records across exits on.
///
/// Anything left behind is the operator's to look at: a record that could not be
/// carried is one they still have on the old stack and do not have on the new, and a
/// script that read success would go on as though the library were whole.
pub(super) fn carrying(report: &lemonfiber_core::model::ImportReport) -> ExitCode {
    if report.refusal.is_some() || !report.not_carried.is_empty() {
        return ExitCode::from(VALIDATION);
    }
    ExitCode::SUCCESS
}

/// What a run under the word `plugin` exits on.
///
/// A read is always a read and always succeeds. An install is the one that can arrive
/// as a report and still be a failure: a proof the service did not hold puts the whole
/// install back, and what comes back is the account of that rather than a refusal — so
/// a script reading the exit code would otherwise be told an install succeeded by a
/// run whose whole subject is that it did not.
///
/// A rehearsal is a reading of what would happen and exits as one. What it reports is
/// that nothing was written, which is what it was asked for.
///
/// A removal is the other run that can arrive as a report and still be a failure: one
/// that could not put everything back has left something on the machine with nothing
/// recording it, and *some of it worked* is the sentence a script must not read as
/// success. What it would have left is not a failure — nothing happened — so a
/// rehearsal exits as the reading it is either way.
pub(super) fn installing(report: &lemonfiber_core::plugin::Installs) -> ExitCode {
    if report
        .removal
        .as_ref()
        .is_some_and(|one| one.removed && !one.went_back.left.is_empty())
    {
        return ExitCode::from(VALIDATION);
    }
    // An update that did not hold is a failure whichever version it left the machine
    // on: a script that asked for the new one and reads success would go on as though
    // it had it.
    if report
        .update
        .as_ref()
        .is_some_and(|one| one.restored.is_some())
    {
        return ExitCode::from(VALIDATION);
    }
    match &report.install {
        None => ExitCode::SUCCESS,
        Some(install) if install.recorded || install.reversed.is_none() => ExitCode::SUCCESS,
        Some(_) => ExitCode::from(VALIDATION),
    }
}

/// What a run that moved the stack onto this build's pinned versions exits on.
///
/// A run that halted part-way left the stack on two versions at once, and a script
/// reading success from it would go on as though everything had moved. What was only
/// shown exits as the reading it is: nothing was touched, so there is nothing for a
/// code to report and no reason to make an operator who is deciding read one.
///
/// A run where every step succeeded and the stack then would not come back is the
/// third case, and it is `Updated` — the update did work. The code still reports the
/// failure, because what a script does next is run against the stack.
pub(super) fn moving(report: &lemonfiber_core::app::update::Report) -> ExitCode {
    use lemonfiber_core::update::State;

    match report.state {
        State::Partial | State::Failed => ExitCode::from(FAILURE),
        // `Updated` is the update having succeeded, which is not the same fact as the
        // stack being up. A run brings back everything it took down for the capture,
        // and a start that would not run leaves that undone and says so here — so a
        // script reading this would otherwise go on against services that are down.
        State::Updated if report.halted.is_some() => ExitCode::from(FAILURE),
        State::Current | State::UpdatesAvailable | State::Updated => ExitCode::SUCCESS,
    }
}

/// What a run that stood in place of a setup already here exits on.
///
/// A refusal is the operator's to resolve. So is a stack left half up: a script that
/// read success from a run which stopped four of six services would go on to start
/// lemonfiber against ports still answered by the other two.
pub(super) fn replacing(report: &lemonfiber_core::model::ReplaceReport) -> ExitCode {
    if report.refusal.is_some() || !report.still_running.is_empty() {
        return ExitCode::from(VALIDATION);
    }
    ExitCode::SUCCESS
}

/// What a run that stood beside a setup already here exits on.
///
/// A refusal is something the operator has to resolve — nowhere left for a service to
/// listen, or a machine that could not be read — so it earns VALIDATION rather than a
/// plain failure.
pub(super) fn standing(report: &lemonfiber_core::model::BesideReport) -> ExitCode {
    if report.refusal.is_some() {
        return ExitCode::from(VALIDATION);
    }
    ExitCode::SUCCESS
}

/// What a run that took over a setup already here exits on.
///
/// A refusal is something the operator has to resolve before lemonfiber will act — a
/// database a later version wrote, or two setups where only they can say which they
/// meant — so it earns VALIDATION rather than a plain failure. Having adopted, and
/// having only said what adopting would come to, are both the command doing what it
/// was asked.
pub(super) fn adopting(report: &AdoptReport) -> ExitCode {
    if report.refusal.is_some() {
        return ExitCode::from(VALIDATION);
    }
    ExitCode::SUCCESS
}

/// What a run that started, stopped or restarted the stack exits on.
///
/// The exit status is the only thing a script reads, and for years this was the one
/// command where it said nothing: every lifecycle outcome sat in the always-success
/// arm below, on the reasoning that whether the stack settled is raised as a problem
/// by the core. It is not. Waiting for services to become usable happens only where
/// Compose exited zero, so a start whose Compose invocation failed raises nothing,
/// returns a report, and used to exit zero — a `lemonfiber up` that started nothing
/// telling its caller it had worked.
///
/// So the Compose status is the verdict. A rehearsal ran nothing and therefore failed
/// at nothing. A status that is absent on a run that was not a rehearsal is a process
/// that was signalled rather than one that exited, which is no more a success than a
/// non-zero code is.
///
/// This is what `pull` has always done — it returns a failure code on a non-zero exit
/// — and the two are the same command in every way that matters to a script.
pub(super) fn lifecycle(report: &LifecycleReport) -> ExitCode {
    if report.rehearsed || report.status == Some(0) {
        return ExitCode::SUCCESS;
    }
    ExitCode::from(FAILURE)
}

/// What a step of setup exits on.
///
/// Recording an answer, moving on and being told where setup stands are all the
/// command doing what it was asked, and an apply that failed already comes back as a
/// problem. One phase is neither: `Applying`, read back out of the progress file,
/// can only mean a previous apply stopped part-way, because one that is still running
/// is the run this answer is waiting on. What is written is written and what is not is
/// not, and until the operator chooses a way out the machine is in neither state.
///
/// So it earns the code a held quality choice earns — something to act on rather than
/// something that went wrong — and a script asking whether this machine is set up can
/// tell "not yet" from "half-way, and somebody has to decide".
pub(super) fn setting_up(report: &WizardReport) -> ExitCode {
    if report.phase == Phase::Applying {
        return ExitCode::from(VALIDATION);
    }
    ExitCode::SUCCESS
}

/// The exit code an accounting of the line earns.
///
/// A reading is always a success, however constrained the line is: being at a cap
/// is a fact about a household's plan and not a fault of the run that said so. A
/// *run that applied limits* is a failure where any client did not take one or is
/// not keeping to it, because that is the case where the operator has a setting
/// they believe in and a household that cannot feel it.
pub(super) fn sharing(report: &lemonfiber_core::bandwidth::Sharing) -> ExitCode {
    if report.applied
        && report
            .clients
            .iter()
            .any(lemonfiber_core::bandwidth::Holding::worth_saying)
    {
        return ExitCode::from(FAILURE);
    }
    ExitCode::SUCCESS
}

/// The exit code a question about the credentials earns.
///
/// Only a rotation that was asked for and did not land is a failure. A reading is a
/// question; a reveal either printed or said why it did not; a rehearsal was never
/// asked to replace anything; and a rotation that landed but left a consumer waiting
/// on a restart is reported in words rather than as a failure, because nothing went
/// wrong — the operator has one more command to run and the report names it.
pub(super) fn rotating(inventory: &lemonfiber_core::credential::Inventory) -> ExitCode {
    match &inventory.rotated {
        // A rehearsal keeps the existing credential and is not a rotation that failed:
        // nothing was attempted, and what came back is the answer that was asked for.
        // Read before the failure below, because it satisfies that test too.
        Some(rotated) if rotated.rehearsed() => ExitCode::SUCCESS,
        Some(rotated) if rotated.kept_the_existing() => ExitCode::from(FAILURE),
        // No rotation was asked for, or one was and it landed. Neither is a fault, so
        // they answer alike rather than through two arms saying the same thing.
        None | Some(_) => ExitCode::SUCCESS,
    }
}

/// The exit code an accounting of the disk earns.
pub(super) fn accounting(report: &lemonfiber_core::space::Reckoning) -> ExitCode {
    match &report.reclaimed {
        None => ExitCode::SUCCESS,
        Some(taken) if taken.left.is_empty() => ExitCode::SUCCESS,
        Some(_) => ExitCode::from(FAILURE),
    }
}

/// The exit code an offer to let one download go earns.
///
/// Named apart from the table for the reason the others here are: an arm that reads
/// an answer is a reading, and a table of readings is one nobody can hold in their
/// head. Unconfirmed is `VALIDATION` rather than success, because the command was
/// asked to stop something seeding and stopped nothing, and a script that read that
/// as done would go on believing a ratio had been given up.
pub(super) fn letting_go(offer: &lemonfiber_core::space::Letting) -> ExitCode {
    if offer.gone.is_some() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(VALIDATION)
    }
}

/// The exit code an uninstall earns.
///
/// A reading and a rehearsal both succeed: neither was asked to remove anything, so
/// neither has failed to. What earns a failure is the one answer a script must not
/// read as done — a removal that ran and left something behind.
pub(super) fn removing_it(removal: &lemonfiber_core::uninstall::Removal) -> ExitCode {
    match removal {
        lemonfiber_core::uninstall::Removal::Surveyed
        | lemonfiber_core::uninstall::Removal::Confirmed
        | lemonfiber_core::uninstall::Removal::Complete { .. } => ExitCode::SUCCESS,
        lemonfiber_core::uninstall::Removal::Partial { .. } => ExitCode::from(FAILURE),
    }
}

/// The exit code a run over what this machine keeps earns.
pub(super) fn forgetting(removal: &lemonfiber_core::stored::Removal) -> ExitCode {
    match removal {
        lemonfiber_core::stored::Removal::NotAsked => ExitCode::SUCCESS,
        lemonfiber_core::stored::Removal::Unconfirmed => ExitCode::from(VALIDATION),
        lemonfiber_core::stored::Removal::Done { left, .. } if left.is_empty() => ExitCode::SUCCESS,
        lemonfiber_core::stored::Removal::Done { .. } => ExitCode::from(FAILURE),
    }
}

/// The exit code taking somebody out of the household earns.
///
/// Unconfirmed earns `VALIDATION` rather than success: the command was asked to remove
/// somebody and removed nobody, and a script that read that as done would carry on as
/// though they were gone. Reaching only the media server earns `FAILURE`, because
/// something is left that the next run has to take — they cannot use it, but it is there.
pub(super) fn removing(report: &lemonfiber_core::model::HouseholdRemoval) -> ExitCode {
    match report.revoked {
        Revoked::Everywhere => ExitCode::SUCCESS,
        Revoked::Nothing => ExitCode::from(VALIDATION),
        Revoked::MediaServerOnly => ExitCode::from(FAILURE),
    }
}

/// The exit code a repairing run earns.
///
/// Anything left unmended is a non-zero result: an operator who asked for things to be put
/// right and had one fail needs their script to know, and a run that offered nothing had
/// nothing wrong it could mend.
pub(crate) fn repairing(report: &RepairReport) -> ExitCode {
    if report.mended.iter().all(|mended| mended.outcome.settled()) {
        return ExitCode::SUCCESS;
    }
    ExitCode::FAILURE
}

/// The exit code a seed earns. Seeding is run to make the wiring true, so leaving any
/// of it unmade is a non-zero result — but the two reasons differ. A refused conflict
/// (two \*arrs on one root folder) is something the operator wrote that lemonfiber will
/// not act on until they resolve it, so it earns VALIDATION; work merely left skipped
/// or failed may complete on a re-run, so it stays FAILURE. A script can then tell "fix
/// your config" from "wait and retry".
pub(crate) fn seed_exit(report: &lemonfiber_core::seed::Report) -> ExitCode {
    // A pass that only said what it would do answered the question it was asked, and
    // every connection it names as outstanding is one nobody has agreed to make yet.
    // Read before completeness, because a rehearsal against a stack with anything left
    // to wire is incomplete by construction — that is the report rather than a fault in
    // it, and a script told otherwise would stop on the answer it asked for.
    if report.rehearsed {
        return ExitCode::SUCCESS;
    }
    if report.is_complete() {
        ExitCode::SUCCESS
    } else if report.blocked().is_empty() {
        ExitCode::from(FAILURE)
    } else {
        ExitCode::from(VALIDATION)
    }
}

/// The exit code a reset earns. A reset without --confirm that found edits to revert
/// only previewed them — like a held quality choice, it needs the operator's say-so, so
/// a script sees a non-zero result to act on. Both an edited stack file and a drifted
/// connection are pending reverts, so either one left unconfirmed is a non-zero result.
/// Confirmed, or with nothing to revert, it succeeded.
pub(crate) fn reset_exit(report: &ResetReport) -> ExitCode {
    let pending = !report.reverted.is_empty() || !report.reverted_connections.is_empty();
    if !report.confirmed && pending {
        ExitCode::from(VALIDATION)
    } else {
        ExitCode::SUCCESS
    }
}

/// The exit code an upgrade earns.
///
/// An unconfirmed upgrade stated its cost and did nothing, so a script sees a non-zero
/// result telling it to confirm. A service that refused is a failure; a run where
/// nothing was actually started — every service still coming up, or none present — is a
/// failure too, so success means at least one re-search began and none was refused.
pub(crate) fn upgrade_exit(report: &UpgradeReport) -> ExitCode {
    let outcome = |want: fn(&Triggered) -> bool| {
        report
            .media
            .iter()
            .filter_map(|media| media.outcome.as_ref())
            .any(want)
    };
    if !report.confirmed {
        ExitCode::from(VALIDATION)
    } else if outcome(|state| matches!(state, Triggered::Failed { .. })) {
        ExitCode::from(FAILURE)
    } else if outcome(|state| matches!(state, Triggered::Started)) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(FAILURE)
    }
}
