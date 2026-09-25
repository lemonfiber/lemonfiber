//! What a run leaves with.
//!
//! An exit code is the only thing a script reads, so deciding one is its own
//! concern rather than a tail on whatever produced the outcome. Every code this
//! binary can return is named here, beside the reasoning for it — a script can
//! branch on *why* something failed rather than merely on whether it did.

use std::process::ExitCode;

mod reporting;

// What each report's own answer comes to. Its own file because that is a different
// question from the one this file asks: here is which code a problem or an outcome
// deserves, there is what one report says about itself.
mod reports;

pub(crate) use reporting::{complain, no_config_home, reported};

use reports::{
    accounting, adopting, carrying, forgetting, installing, letting_go, lifecycle, moving,
    removing, removing_it, replacing, rotating, setting_up, sharing, standing,
};
pub(crate) use reports::{repairing, reset_exit, seed_exit, upgrade_exit};

use lemonfiber_core::app::Outcome;
use lemonfiber_core::doctor::Overall;
use lemonfiber_core::error::codes::{leaves, Leaves};
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{Disposition, Triggered};

/// A general failure. Codes are meaningful so a script can branch on *why*
/// something failed rather than merely on whether it did.
pub(crate) const FAILURE: u8 = 1;

/// A flag or argument the operator gave could not be understood.
pub(crate) const USAGE: u8 = 2;

/// Something outside lemonfiber has to be fixed before it can act.
pub(crate) const PREFLIGHT: u8 = 3;

/// Started, and a service never became usable.
pub(crate) const NEVER_SETTLED: u8 = 4;

/// Something the operator wrote was refused.
pub(crate) const VALIDATION: u8 = 5;

/// Which exit code a problem deserves.
///
/// A script branching on failure needs to know whether to fix its own input,
/// start Docker, or wait longer, and one code for all three tells it nothing.
/// Which of those a code means is declared beside the code, in the registry.
pub(crate) fn exit_code(problem: &Problem) -> u8 {
    match leaves(problem.code) {
        Leaves::NeverSettled => NEVER_SETTLED,
        Leaves::Preflight => PREFLIGHT,
        Leaves::Validation => VALIDATION,
        Leaves::Failure => FAILURE,
    }
}

/// Most answers are simply produced, so their success is that they arrived. A
/// diagnosis is different: a script runs it precisely to learn whether the stack
/// is healthy, so a broken or undetermined result must exit non-zero — reporting
/// success when nothing could be verified is the falsehood this product exists to
/// avoid.
pub(crate) fn settled(outcome: &Outcome) -> ExitCode {
    match outcome {
        Outcome::Doctor(report) => match report.overall {
            Overall::Healthy | Overall::Degraded => ExitCode::SUCCESS,
            Overall::Broken | Overall::Unknown => ExitCode::from(FAILURE),
        },
        // Seeding is run to make the wiring true, so leaving any of it unmade is a
        // non-zero result — but the two reasons differ. A refused conflict (two
        // \*arrs on one root folder) is something the operator wrote that lemonfiber
        // will not act on until they resolve it, so it earns VALIDATION; work merely
        // left skipped or failed may complete on a re-run, so it stays FAILURE. A
        // script can then tell "fix your config" from "wait and retry".
        Outcome::Adoption(report) => adopting(report),
        Outcome::Beside(report) => standing(report),
        Outcome::Replacement(report) => replacing(report),
        // An ask nothing fills is a stack that will not wire, and a script asking
        // what this stack wires to what is asking exactly that. It is the operator's
        // own configuration to fix, which is the code that says so.
        Outcome::Plugins(report) => installing(report),
        Outcome::Wiring(report) if report.unfilled.is_empty() => ExitCode::SUCCESS,
        Outcome::Wiring(_) => ExitCode::from(VALIDATION),
        Outcome::Import(report) => carrying(report),
        Outcome::Seed(report) => seed_exit(report),
        // Anything left unmended is a non-zero result, and a run that only offered
        // has mended everything it carried out — which is none of it.
        Outcome::Repair(report) => repairing(report),
        // A held quality choice was not recorded — it needs the operator to
        // confirm a preset this machine would software-transcode, so a script sees
        // a non-zero result it can act on rather than a false success.
        Outcome::Quality(report) => match report.disposition {
            Disposition::Held => ExitCode::from(VALIDATION),
            Disposition::Shown
            | Disposition::Recorded
            | Disposition::Rehearsed
            | Disposition::Reapplied
            | Disposition::WouldReapply => ExitCode::SUCCESS,
        },
        Outcome::Upgrade(report) => upgrade_exit(report),
        // The music choice is recorded even when the service could not be reached, so
        // only a service that refused the change is a failure; a rehearsal or a service
        // still coming up has still recorded the choice.
        Outcome::Music(report) => {
            if matches!(report.outcome, Some(Triggered::Failed { .. })) {
                ExitCode::from(FAILURE)
            } else {
                ExitCode::SUCCESS
            }
        }
        // A reset run without --confirm that found edits to revert only previewed them —
        // like a held quality choice, it needs the operator's say-so, so a script sees a
        // non-zero result to act on. Both an edited stack file and a drifted connection
        // are pending reverts, so either one left unconfirmed is a non-zero result.
        // Confirmed, or with nothing to revert, it succeeded.
        Outcome::Reset(report) => reset_exit(report),
        // Taking somebody out of the household has the same three answers a forget
        // does. Unconfirmed is waiting on the operator's say-so, so a script reading
        // success would carry on as though the person were gone; a removal that reached
        // only the media server left an account behind that the next run has to take.
        Outcome::Removal(report) => removing(report),
        // A listing is a question and asking is never a failure. A removal that was
        // not confirmed is waiting on the operator's say-so, and one that could not
        // take a directory left something behind — a script that read either as
        // success would carry on as though the machine were clean.
        // Listing the credentials is a question, and asking one is never a failure —
        // including when the answer is that several have gone stale, which is an
        // advisory rather than a fault. What is a failure is a replacement that was
        // asked for and did not happen: a script that read that as success would go on
        // believing a credential had been rotated when the old one is still in force.
        Outcome::Credentials(inventory) => rotating(inventory),
        Outcome::Stored(report) => forgetting(&report.removal),
        // Accounting for the disk is a question, and asking one is never a failure —
        // including when the answer is that there is no room, which the report says
        // in words and which the commands that would fetch more refuse over. What is
        // a failure is a cleanup that was agreed to and could not finish.
        Outcome::Space(report) => accounting(report),
        Outcome::StopSeeding(offer) => letting_go(offer),
        // A reading is a question and asking one is never a failure. A removal that
        // could not take everything left something behind, and a script that read
        // that as success would carry on believing the machine was clean.
        Outcome::Uninstall(report) => removing_it(&report.removal),
        // Accounting for the line is a question too, and one answer to it is a
        // failure a script has to be able to see: a limit that was handed to a
        // client and did not take is a setting the operator believes is in force
        // while the household's evening goes on being ruined.
        Outcome::Bandwidth(report) => sharing(report),
        // A restore that overwrote nothing listed what it would overwrite and
        // stopped — like an unconfirmed reset, it is waiting on the operator's
        // say-so, so a script sees a non-zero result rather than a false success.
        Outcome::Restore(report) => {
            if report.done.is_some() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(VALIDATION)
            }
        }
        Outcome::Update(report) => moving(report),
        Outcome::Lifecycle(report) => lifecycle(report),
        Outcome::Wizard(report) => setting_up(report),
        // Only a walk that stopped is a failure. One that finished worked; one still
        // downloading is working, and reporting that as a failure would contradict
        // the sentence that just told the operator nothing was cancelled; and one
        // that found the content already here answered the question it was asked.
        Outcome::Walkthrough(report) => {
            if report.state.is_a_problem() {
                ExitCode::from(FAILURE)
            } else {
                ExitCode::SUCCESS
            }
        }
        // A trace, a stuck-item listing, the household's requests or where the
        // household begins is a query — it answers where things are; asking is never a
        // failure, whatever the answer. A stack with no front door has been asked and
        // answered, so the answer arrives as one rather than as a code.
        //
        // A guard that ended is one too. It ended because the data location went,
        // which is the thing it was watching for, and it reports whether it got the
        // services stopped — so the report is the answer rather than the failure.
        Outcome::Version(_)
        | Outcome::Forms(_)
        | Outcome::Preview(_)
        | Outcome::Config(_)
        // What the operator is told about was reported or changed; a write that could
        // not happen already comes back as a problem.
        | Outcome::Alerts(_)
        | Outcome::Migration(_)
        // The record arrived. That a change on it cannot be put back is what the read
        // was asked, not a failure to answer.
        | Outcome::History(_)
        | Outcome::Trace(_)
        | Outcome::Household(_)
        // The shelf arrived. That the media server would not say what is on it is
        // reported in the answer, for the same reason it is on the household read.
        | Outcome::Held(_)
        // What is hosted is a reading, and an install or a removal that could not be
        // carried out already comes back as a problem — so a report here is one that
        // arrived, whatever it says stands.
        | Outcome::Hosting(_)
        | Outcome::FrontDoor(_)
        | Outcome::Stuck(_)
        | Outcome::Status(_)
        | Outcome::Word(_)
        | Outcome::Glossary(_)
        | Outcome::Clients(_)
        // Where this copy stands is a reading too, and one that must never be a
        // failure: an availability check a script read as a non-zero result would be
        // this product making its own currency a precondition of running.
        | Outcome::SelfUpdate(_)
        // An invitation was made or it was not; a refusal already comes back as a
        // problem, so there is nothing for a code to tell apart here.
        | Outcome::Invitation(_)
        | Outcome::Outbound(_)
        | Outcome::Provenance(_)
        | Outcome::Catalogue(_)
        // A substitution was recorded, or worked out and not written; one that
        // could not be made comes back as a problem.
        | Outcome::Substitution(_)
        // Putting back what the last repair changed either happened or came back as
        // a problem; there is no third answer for a code to distinguish.
        | Outcome::Undo(_)
        // A capture that was written and a bundle that was described are each an
        // answer that arrived, and a run that could not produce one comes back as
        // a problem rather than as an outcome with a code on it.
        | Outcome::Backup(_)
        | Outcome::Watch(_)
        | Outcome::Archives(_)
        | Outcome::Bundle(_) => ExitCode::SUCCESS,
    }
}

/// An exit code as it reads, so two can be compared.
///
/// `ExitCode` implements neither `PartialEq` nor `Debug`-free comparison, so every test
/// that checks one renders it first. Written once here — the module that owns exit codes —
/// rather than re-spelled in each of the seven that check them.
#[cfg(test)]
pub(crate) fn shown(code: std::process::ExitCode) -> String {
    format!("{code:?}")
}

/// A clean exit, as it reads.
#[cfg(test)]
pub(crate) fn success() -> String {
    shown(std::process::ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests;
