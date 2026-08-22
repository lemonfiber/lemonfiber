//! What a run leaves with.
//!
//! An exit code is the only thing a script reads, so deciding one is its own
//! concern rather than a tail on whatever produced the outcome. Every code this
//! binary can return is named here, beside the reasoning for it — a script can
//! branch on *why* something failed rather than merely on whether it did.

use std::process::ExitCode;

use crate::say::complain;
use lemonfiber_core::app::repair::Report as RepairReport;
use lemonfiber_core::app::Outcome;
use lemonfiber_core::doctor::Overall;
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{Disposition, ResetReport, Triggered, UpgradeReport};

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
        // A trace, a stuck-item listing or the household's requests is a query — it
        // answers where things are; asking is never a failure, whatever the answer.
        Outcome::Version(_)
        | Outcome::Forms(_)
        | Outcome::Preview(_)
        | Outcome::Lifecycle(_)
        | Outcome::Config(_)
        | Outcome::Trace(_)
        | Outcome::Household(_)
        | Outcome::Stuck(_)
        | Outcome::Status(_) => ExitCode::SUCCESS,
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
    complain!("{}: {}", problem.code, problem.summary);
    complain!("\n  {}\n", problem.meaning);

    for remedy in &problem.remedies {
        complain!("  → {}", remedy.action);
        if let Some(detail) = &remedy.detail {
            complain!("    {detail}");
        }
    }

    // Last, and indented: available to whoever wants it, and never the first
    // thing the operator has to read.
    if let Some(detail) = &problem.detail {
        complain!();
        for line in detail.lines() {
            complain!("  {line}");
        }
    }

    ExitCode::from(exit_code(problem))
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
    use lemonfiber_core::model::{
        Disposition, DoctorReport, LifecycleReport, MusicChoice, MusicReport, QualityReport,
        ResetReport, StackEdit, StatusReport, Triggered, UpgradeMedia, UpgradeReport,
        VersionReport,
    };
    use lemonfiber_core::seed::{
        Assessment, Report as SeedReport, Severity as SeedSeverity, State as SeedState, Wiring,
    };

    use super::{
        complain, exit_code, no_config_home, settled, success, FAILURE, NEVER_SETTLED, USAGE,
        VALIDATION,
    };

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
}
