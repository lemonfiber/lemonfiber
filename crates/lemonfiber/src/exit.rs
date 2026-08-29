//! What a run leaves with.
//!
//! An exit code is the only thing a script reads, so deciding one is its own
//! concern rather than a tail on whatever produced the outcome. Every code this
//! binary can return is named here, beside the reasoning for it — a script can
//! branch on *why* something failed rather than merely on whether it did.

use std::process::ExitCode;

use crate::render::Lines;
use crate::say::complain;
use lemonfiber_core::app::repair::Report as RepairReport;
use lemonfiber_core::app::Outcome;
use lemonfiber_core::doctor::Overall;
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::kind;
use lemonfiber_core::model::{Disposition, Envelope, ResetReport, Triggered, UpgradeReport};

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
pub(crate) fn exit_code(problem: &Problem) -> u8 {
    use lemonfiber_core::{app, config, ports, stack};

    match problem.code {
        app::NEVER_SETTLED => NEVER_SETTLED,
        ports::process::MISSING_PROGRAM | ports::docker::ENGINE_UNREACHABLE => PREFLIGHT,
        stack::STACK_INVALID | stack::STACK_UNREADABLE | config::store::CONFIG_UNREADABLE => {
            VALIDATION
        }
        _ => FAILURE,
    }
}

/// The exit code an outcome deserves.
///
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
        // A listing is a question and asking is never a failure. A removal that was
        // not confirmed is waiting on the operator's say-so, and one that could not
        // take a directory left something behind — a script that read either as
        // success would carry on as though the machine were clean.
        Outcome::Stored(report) => forgetting(&report.removal),
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
        | Outcome::Lifecycle(_)
        | Outcome::Config(_)
        | Outcome::Trace(_)
        | Outcome::Household(_)
        | Outcome::FrontDoor(_)
        | Outcome::Stuck(_)
        | Outcome::Status(_)
        | Outcome::Word(_)
        | Outcome::Glossary(_)
        | Outcome::Clients(_)
        // An invitation was made or it was not; a refusal already comes back as a
        // problem, so there is nothing for a code to tell apart here.
        | Outcome::Invited(_)
        | Outcome::Outbound(_)
        | Outcome::Wizard(_)
        // Putting back what the last repair changed either happened or came back as
        // a problem; there is no third answer for a code to distinguish.
        | Outcome::Undo(_)
        // A capture that was written and a bundle that was described are each an
        // answer that arrived, and a run that could not produce one comes back as
        // a problem rather than as an outcome with a code on it.
        | Outcome::Backup(_)
        | Outcome::Watch(_)
        | Outcome::Archives(_)
        | Outcome::Support(_) => ExitCode::SUCCESS,
    }
}

/// The exit code a run over what this machine keeps earns.
fn forgetting(removal: &lemonfiber_core::stored::Removal) -> ExitCode {
    match removal {
        lemonfiber_core::stored::Removal::NotAsked => ExitCode::SUCCESS,
        lemonfiber_core::stored::Removal::Unconfirmed => ExitCode::from(VALIDATION),
        lemonfiber_core::stored::Removal::Done { left, .. } if left.is_empty() => ExitCode::SUCCESS,
        lemonfiber_core::stored::Removal::Done { .. } => ExitCode::from(FAILURE),
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

/// Tell the operator what went wrong, and exit in a way a script can branch on.
///
/// One renderer, so a failure reads the same whichever command produced it —
/// the remedies are the point of the error model, and a second copy of this is
/// how one of them quietly starts omitting them.
pub(crate) fn complain(problem: &Problem) -> ExitCode {
    reported(problem, crate::say::for_a_parser()).eprint();
    ExitCode::from(exit_code(problem))
}

/// What an operator is told about a failure.
///
/// Built rather than printed, which every other answer in this crate already did
/// and this one did not. Three things follow, and only the first was the reason.
///
/// **The text is made safe on the way out.** `Lines::put` passes everything through
/// [`lemonfiber_core::text::plain`], and a failure carries text this product did not
/// write — a service’s own words, a filesystem’s reason. A terminal is not a text
/// box: an escape in the middle of one clears the screen or writes over the line
/// just printed, and a diagnosis that no longer says what this product said has lost
/// the whole of what it was for. The redaction a detail already passed through looks
/// for credentials, not for instructions.
///
/// **A remedy is rendered the one way.** [`Lines::remedy`] exists so that a
/// diagnosis, a repair’s escalation and this cannot drift on how an action and its
/// detail sit together — and this had drifted, by writing the identical shape by
/// hand with nothing keeping the two the same.
///
/// **And a failure explains its own words**, like every other answer, which matters
/// most here: an error is where somebody is least able to go and look one up.
pub(crate) fn reported(problem: &Problem, parsed: bool) -> Lines {
    if parsed {
        return as_a_document(problem);
    }

    let mut lines = Lines::default();
    lines.put(format!("{}: {}", problem.code, problem.summary));
    lines.put("");
    lines.put(format!("  {}", problem.meaning));
    lines.put("");

    for remedy in &problem.remedies {
        lines.remedy(remedy, "  ");
    }

    // Last, and indented: available to whoever wants it, and never the first
    // thing the operator has to read.
    if let Some(detail) = &problem.detail {
        lines.put("");
        for line in detail.lines() {
            lines.put(format!("  {line}"));
        }
    }

    let notes = crate::render::glossary::footnotes(
        &lines.text(),
        crate::render::glossary::wanted(),
        crate::render::glossary::known(),
    );
    lines.extend(notes);
    lines
}

/// The same failure, for something that will parse it.
///
/// One document rather than several lines of prose, and on the error stream still:
/// what a run was asked for goes on standard output, and what went wrong instead
/// belongs beside it rather than in it — a script reading the answer should not have
/// to tell an answer from an apology.
///
/// A script that asked for output it could parse asked about the failures too. They
/// are the answers it most needs to act on, and an exit code alone says that
/// something went wrong without saying what.
fn as_a_document(problem: &Problem) -> Lines {
    let mut lines = Lines::for_a_parser();
    lines.put(
        Envelope::new(kind::ERROR, problem)
            .to_json()
            // Eagerly, for the reason `machine_readable` states beside it: a
            // lazily-built fallback is a line no test could ever run, since these
            // payloads are plain data that cannot fail to serialise.
            .unwrap_or(crate::render::UNRENDERABLE.to_owned()),
    );
    lines
}

/// Where this machine keeps lemonfiber's files.
///
/// Finding the platform's base directories is the surface's job: it means asking
/// the operating system, and there is nothing about it a test could catch that
/// running it would not. The layout beneath those bases is the core's, and is
/// tested there.
/// Refuse an operation that needs to know where the configuration is kept, when
/// this platform will not say. The one message for it, so both callers word it the
/// same way.
pub(crate) fn no_config_home() -> ExitCode {
    complain!("error: lemonfiber could not find where its configuration is kept");
    ExitCode::FAILURE
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
mod tests {
    use lemonfiber_core::app::Outcome;
    use lemonfiber_core::doctor::Overall;
    use lemonfiber_core::error::{Code, Problem, Remedy, Severity, State};
    use lemonfiber_core::model::WalkthroughReport;
    use lemonfiber_core::model::{
        Disposition, DoctorReport, LifecycleReport, MusicChoice, MusicReport, QualityReport,
        ResetReport, StackEdit, StatusReport, Triggered, UpgradeMedia, UpgradeReport,
        VersionReport,
    };
    use lemonfiber_core::seed::{
        Assessment, Report as SeedReport, Severity as SeedSeverity, State as SeedState, Wiring,
    };
    use lemonfiber_core::stored::{stored, Left, Removal};
    use lemonfiber_core::walkthrough::{Shape, State as WalkState};

    use lemonfiber_core::config::paths::Paths;
    use std::path::Path;

    use super::{
        complain, exit_code, no_config_home, settled, shown, success, FAILURE, NEVER_SETTLED,
        USAGE, VALIDATION,
    };

    /// A walk that ended in one state, with nothing else to say about it.
    fn walked(state: WalkState) -> Outcome {
        Outcome::Walkthrough(WalkthroughReport {
            shape: Shape::Pipeline,
            state,
            proves: String::new(),
            item: None,
            lines: Vec::new(),
            stopped: None,
            link: None,
            handover: None,
            suggestions: Vec::new(),
            in_background: false,
            already_here: false,
        })
    }

    /// A problem of the given severity and state.
    fn problem(severity: Severity, state: State) -> Problem {
        let mut problem = Problem::new(
            Code::new("TEST"),
            severity,
            "it broke",
            "nothing imports",
            Remedy::new("restart it"),
        );
        problem.state = state;
        problem
    }

    fn lifecycle(status: Option<i32>) -> LifecycleReport {
        LifecycleReport {
            status,
            ..crate::render::fixtures::a_lifecycle(
                "up",
                crate::render::fixtures::a_plan("tv", Vec::new()),
            )
        }
    }

    #[test]
    fn a_problem_the_operator_wrote_is_told_apart_from_one_they_can_only_report() {
        // The code decides, not the severity: a script branches on *why* rather than
        // on how loudly. Something the operator wrote earns VALIDATION, something
        // outside lemonfiber earns PREFLIGHT, and the rest is a plain failure.
        let coded = |code| {
            let mut it = problem(Severity::Error, State::Guided);
            it.code = code;
            exit_code(&it)
        };
        assert_eq!(coded(lemonfiber_core::stack::STACK_INVALID), VALIDATION);
        assert_eq!(
            coded(lemonfiber_core::config::store::CONFIG_UNREADABLE),
            VALIDATION
        );
        assert_eq!(
            coded(lemonfiber_core::ports::docker::ENGINE_UNREACHABLE),
            super::PREFLIGHT
        );
        assert_eq!(coded(lemonfiber_core::app::NEVER_SETTLED), NEVER_SETTLED);
        assert_eq!(coded(Code::new("SOMETHING-ELSE")), FAILURE);
    }

    #[test]
    fn a_diagnosis_exits_on_what_it_found_rather_than_on_having_run() {
        // A script runs `doctor` precisely to learn whether the stack is healthy, so
        // reporting success when nothing could be verified is the falsehood this
        // product exists to avoid.
        for (overall, healthy) in [
            (Overall::Healthy, true),
            (Overall::Degraded, true),
            (Overall::Broken, false),
            (Overall::Unknown, false),
        ] {
            let code = settled(&Outcome::Doctor(DoctorReport {
                overall,
                findings: Vec::new(),
            }));
            assert_eq!(format!("{code:?}") == success(), healthy, "{overall:?}");
        }
    }

    #[test]
    fn a_lifecycle_reports_what_it_did_rather_than_a_verdict_on_it() {
        // Whether the stack settled is a property of the run, raised as a problem by
        // the core; the report of what was done is not itself a pass or a fail.
        for status in [Some(0), Some(1), None] {
            assert_eq!(
                format!("{:?}", settled(&Outcome::Lifecycle(lifecycle(status)))),
                success(),
                "{status:?}"
            );
        }
    }

    #[test]
    fn a_held_quality_choice_is_a_validation_result_rather_than_a_failure() {
        // Held means the operator has to say so explicitly, which is something they
        // can act on rather than something that went wrong.
        let held = QualityReport {
            choices: Vec::new(),
            music: None,
            customised: false,
            disposition: Disposition::Held,
        };
        assert_ne!(format!("{:?}", settled(&Outcome::Quality(held))), success());
        let shown = QualityReport {
            choices: Vec::new(),
            music: None,
            customised: false,
            disposition: Disposition::Shown,
        };
        assert_eq!(
            format!("{:?}", settled(&Outcome::Quality(shown))),
            success()
        );
    }

    #[test]
    fn an_upgrade_that_no_service_started_is_not_a_success() {
        let started = UpgradeReport {
            confirmed: true,
            media: vec![UpgradeMedia {
                media_type: "tv".to_owned(),
                preset: "Balanced".to_owned(),
                size_per_hour: "3 GB".to_owned(),
                outcome: Some(Triggered::Started),
            }],
        };
        assert_eq!(
            format!("{:?}", settled(&Outcome::Upgrade(started))),
            success()
        );
        let refused = UpgradeReport {
            confirmed: true,
            media: vec![UpgradeMedia {
                media_type: "tv".to_owned(),
                preset: "Balanced".to_owned(),
                size_per_hour: "3 GB".to_owned(),
                outcome: Some(Triggered::Failed {
                    detail: "boom".to_owned(),
                }),
            }],
        };
        assert_ne!(
            format!("{:?}", settled(&Outcome::Upgrade(refused))),
            success()
        );
    }

    /// Four answers over one shape, and each is a different thing for a script to do.
    /// A listing is a question; an unconfirmed removal is waiting to be told to go
    /// ahead; a removal that took everything is done; and one that left a directory
    /// behind has left the machine not clean, which is the case a script reading
    /// success would carry on past.
    #[test]
    fn what_this_machine_keeps_answers_four_ways_and_only_one_of_them_is_success() {
        let layout = Paths::rooted(Path::new("/scratch/config"), Path::new("/scratch/data"));
        let asked = |removal| format!("{:?}", settled(&Outcome::Stored(stored(&layout, removal))));

        assert_eq!(asked(Removal::NotAsked), success());
        assert_ne!(asked(Removal::Unconfirmed), success());
        assert_eq!(
            asked(Removal::Done {
                gone: vec!["/scratch/config/lemonfiber".to_owned()],
                left: Vec::new(),
            }),
            success()
        );
        assert_ne!(
            asked(Removal::Done {
                gone: Vec::new(),
                left: vec![Left {
                    at: "/scratch/data/lemonfiber".to_owned(),
                    why: "permission denied".to_owned(),
                }],
            }),
            success()
        );
    }

    #[test]
    fn an_unconfirmed_reset_that_found_edits_asks_to_be_confirmed() {
        let pending = ResetReport {
            reverted: vec![StackEdit {
                path: "compose.yml".to_owned(),
                diff: String::new(),
            }],
            reverted_connections: Vec::new(),
            confirmed: false,
        };
        assert_ne!(
            format!("{:?}", settled(&Outcome::Reset(pending))),
            success()
        );
        let nothing = ResetReport {
            reverted: Vec::new(),
            reverted_connections: Vec::new(),
            confirmed: false,
        };
        assert_eq!(
            format!("{:?}", settled(&Outcome::Reset(nothing))),
            success()
        );
    }

    #[test]
    fn a_seed_that_left_work_undone_says_so_and_a_conflict_says_it_differently() {
        let settled_seed = SeedReport {
            wirings: vec![Wiring {
                connection: "a".to_owned(),
                state: SeedState::Wired,
                severity: SeedSeverity::Informational,
            }],
            assessment: Assessment::Assessed,
        };
        assert_eq!(
            format!("{:?}", settled(&Outcome::Seed(settled_seed))),
            success()
        );
        // Something the operator wrote that lemonfiber will not act on until they
        // resolve it earns a different code from work that may simply complete later.
        let blocked = SeedReport {
            wirings: vec![Wiring {
                connection: "a".to_owned(),
                state: SeedState::Refused {
                    reason: "two arrs".to_owned(),
                },
                severity: SeedSeverity::Informational,
            }],
            assessment: Assessment::Assessed,
        };
        assert_ne!(format!("{:?}", settled(&Outcome::Seed(blocked))), success());
    }

    #[test]
    fn a_music_choice_still_recorded_is_a_success_however_the_service_answered() {
        // Recorded is the point; reaching the service is a bonus a later run can
        // still deliver, so only an outright refusal is a failure.
        let choice = MusicChoice {
            scope: "music".to_owned(),
            format: "FLAC".to_owned(),
            means: "lossless".to_owned(),
            targets: "albums".to_owned(),
            size_per_hour: "400 MB".to_owned(),
            note: "large".to_owned(),
        };
        for outcome in [None, Some(Triggered::Started), Some(Triggered::NotStarted)] {
            let report = MusicReport {
                choice: choice.clone(),
                disposition: Disposition::Recorded,
                outcome,
            };
            assert_eq!(format!("{:?}", settled(&Outcome::Music(report))), success());
        }
    }

    #[test]
    fn a_seed_left_only_with_work_to_retry_is_told_from_one_with_a_conflict() {
        // Skipped work may complete on a re-run; a refused conflict will not until
        // the operator resolves it, so a script can tell "wait" from "fix".
        let waiting = SeedReport {
            wirings: vec![Wiring {
                connection: "a".to_owned(),
                state: SeedState::Skipped {
                    reason: "not up".to_owned(),
                },
                severity: SeedSeverity::Informational,
            }],
            assessment: Assessment::Assessed,
        };
        assert_ne!(format!("{:?}", settled(&Outcome::Seed(waiting))), success());
    }

    #[test]
    fn an_upgrade_nobody_confirmed_and_one_nothing_answered_are_both_unfinished() {
        let media = |outcome| UpgradeMedia {
            media_type: "tv".to_owned(),
            preset: "Balanced".to_owned(),
            size_per_hour: "3 GB".to_owned(),
            outcome,
        };
        // Stated but not done.
        let unconfirmed = UpgradeReport {
            confirmed: false,
            media: vec![media(None)],
        };
        assert_ne!(
            format!("{:?}", settled(&Outcome::Upgrade(unconfirmed))),
            success()
        );
        // Confirmed, but no service was up to start anything.
        let silent = UpgradeReport {
            confirmed: true,
            media: vec![media(Some(Triggered::NotStarted))],
        };
        assert_ne!(
            format!("{:?}", settled(&Outcome::Upgrade(silent))),
            success()
        );
    }

    #[test]
    fn a_music_choice_is_recorded_even_where_the_service_refused_it() {
        let refused = MusicReport {
            choice: MusicChoice {
                scope: "music".to_owned(),
                format: "FLAC".to_owned(),
                means: "lossless".to_owned(),
                targets: "albums".to_owned(),
                size_per_hour: "400 MB".to_owned(),
                note: "large".to_owned(),
            },
            disposition: Disposition::Recorded,
            outcome: Some(Triggered::Failed {
                detail: "refused".to_owned(),
            }),
        };
        assert_ne!(
            format!("{:?}", settled(&Outcome::Music(refused))),
            success()
        );
    }

    #[test]
    fn asking_where_something_is_is_never_a_failure_whatever_the_answer() {
        // A query answers; it does not succeed or fail.
        for outcome in [
            Outcome::Version(VersionReport {
                binary: "0.4.0".to_owned(),
                supported_schema: vec![1],
                stack: "1".to_owned(),
                compose: None,
            }),
            Outcome::Status(StatusReport {
                forms: Vec::new(),
                condition: lemonfiber_core::docker::Condition::Inactive,
                services: Vec::new(),
            }),
        ] {
            assert_eq!(format!("{:?}", settled(&outcome)), success());
        }
    }

    #[test]
    fn a_problem_is_reported_with_its_remedies_and_a_missing_home_with_its_own_words() {
        let mut carrying = problem(Severity::Error, State::Guided);
        carrying.detail = Some("the log said so".to_owned());
        carrying.remedies = vec![Remedy::new("restart it").with_detail("compose restart")];
        assert_ne!(format!("{:?}", complain(&carrying)), success());
        assert_ne!(format!("{:?}", no_config_home()), success());
        let _ = USAGE;
    }

    #[test]
    fn a_restore_that_overwrote_nothing_is_not_reported_as_a_restore() {
        // The listing is what a run that has not been confirmed produces, and a
        // script told it succeeded would believe the archive had been put back.
        use lemonfiber_core::app::restore::{Preview, Report as Restored, Restoration};
        use lemonfiber_core::backup::{Manifest, Scope, SCHEMA};

        let would = Preview {
            manifest: Manifest {
                schema: SCHEMA,
                product_version: "0.7.0".to_owned(),
                created_at: "2026-07-30".to_owned(),
                data_root: "/srv/media".to_owned(),
                scope: Scope::WholeStack,
                sensitive: true,
                members: Vec::new(),
            },
            downgrade: false,
            relocation: None,
            agreement: "5c3a1d20".to_owned(),
        };
        assert_eq!(
            shown(settled(&Outcome::Restore(Restoration {
                would: would.clone(),
                done: None,
            }))),
            shown(std::process::ExitCode::from(VALIDATION))
        );
        assert_eq!(
            shown(settled(&Outcome::Restore(Restoration {
                would,
                done: Some(Restored {
                    scope: Scope::WholeStack,
                    from_version: "0.7.0".to_owned(),
                    relocated: None,
                }),
            }))),
            success()
        );
    }

    #[test]
    fn a_capture_and_a_bundle_succeed_by_having_arrived() {
        use lemonfiber_core::app::backup::Report as Capture;
        use lemonfiber_core::app::support::Bundle;
        use lemonfiber_core::backup::Scope;
        use lemonfiber_core::bundle::Contents;

        assert_eq!(
            shown(settled(&Outcome::Backup(Capture {
                path: std::path::PathBuf::from("/data/lemonfiber/backups/full.tar.gz"),
                scope: Scope::WholeStack,
                sensitive: true,
                pruned: Vec::new(),
            }))),
            success()
        );
        assert_eq!(
            shown(settled(&Outcome::Support(Bundle {
                contents: Contents::default(),
                bytes: 0,
                path: None,
            }))),
            success()
        );
    }

    #[test]
    fn only_a_walk_that_stopped_is_a_failure() {
        // One that finished worked; one still downloading is working, and calling
        // that a failure would contradict the sentence that has just told the
        // operator nothing was cancelled; and one that found the content already
        // here answered the question it was asked.
        assert_eq!(shown(settled(&walked(WalkState::Complete))), success());
        assert_eq!(shown(settled(&walked(WalkState::Downloading))), success());
        assert_eq!(shown(settled(&walked(WalkState::Skipped))), success());
        assert_ne!(shown(settled(&walked(WalkState::Failed))), success());
    }

    #[test]
    fn a_guard_that_ended_is_a_report_rather_than_a_failure() {
        // It ended because the data location went, which is what it was watching
        // for, and it says whether it got the services stopped.
        let stranded = Outcome::Watch(lemonfiber_core::model::SupervisionReport {
            forms: vec!["library".to_owned()],
            reason: "the data location is no longer present".to_owned(),
            stopped: false,
        });
        assert_eq!(shown(settled(&stranded)), success());
    }
}

#[cfg(test)]
mod reporting {
    use super::reported;
    use lemonfiber_core::error::{Code, Problem, Remedy, Severity};

    /// A failure carries text this product did not write, and a terminal is not a
    /// text box: one escape clears the screen, another writes over the line just
    /// printed. Nothing runs, and the screen stops saying what this product said —
    /// which, for a diagnosis, is the whole of what it was for.
    ///
    /// Every other answer already went out through `text::plain`. This one did not,
    /// and the redaction it *did* pass through looks only for credentials.
    #[test]
    fn a_failure_cannot_carry_an_instruction_to_the_terminal() {
        let escape = char::from(27);
        let problem = Problem::new(
            Code::new("WORD-9"),
            Severity::Error,
            format!("the service refused{escape}[2J"),
            "it gave a reason of its own.",
            Remedy::new("Try again"),
        )
        .with_detail(format!("HTTP 500: {escape}[H database is locked"));

        let said = reported(&problem, false).text();

        assert!(
            !said.contains(escape),
            "an escape reached the screen: {said:?}"
        );
        assert!(
            said.contains("the service refused"),
            "and the words survived: {said}"
        );
        assert!(said.contains("database is locked"), "{said}");
    }

    /// The words a failure uses are explained like any other answer’s, which matters
    /// most here: an error is where somebody is least able to go and look one up.
    #[test]
    fn a_failure_explains_its_own_words() {
        let problem = Problem::new(
            Code::new("WORD-8"),
            Severity::Error,
            "no indexer answered in time",
            "nothing could be searched for.",
            Remedy::new("Check the indexer is reachable"),
        );

        let said = reported(&problem, false).text();

        assert!(said.contains("Words used here:"), "{said}");
        assert!(said.contains("indexer — Search engines"), "{said}");
    }

    /// A script that asked for output it could parse asked about the failures too.
    /// They are the answers it most needs to act on, and an exit code alone says
    /// that something went wrong without saying what.
    ///
    /// Asked of the decision rather than of the latch, so this settles nothing for
    /// the tests beside it.
    #[test]
    fn a_failure_a_script_asked_for_is_one_document_it_can_parse() {
        let problem = Problem::new(
            Code::new("WORD-7"),
            Severity::Error,
            "no indexer answered in time",
            "nothing could be searched for.",
            Remedy::new("Check the indexer is reachable"),
        );

        let said = reported(&problem, true).text();

        assert_eq!(said.lines().count(), 1, "one document, not several: {said}");
        assert!(said.starts_with("{\"api_version\""), "{said}");
        assert!(said.contains("\"kind\":\"error\""), "{said}");
        assert!(said.contains("no indexer answered in time"), "{said}");
        assert!(
            !said.contains("Words used here:"),
            "and nothing a person would want in it: {said}"
        );
    }
}
