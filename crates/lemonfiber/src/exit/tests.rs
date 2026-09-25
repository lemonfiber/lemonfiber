use lemonfiber_core::app::Outcome;
use lemonfiber_core::doctor::Overall;
use lemonfiber_core::error::{Code, Problem, Remedy, Severity, State};
use lemonfiber_core::model::WalkthroughReport;
use lemonfiber_core::model::{
    Disposition, DoctorReport, LifecycleReport, MusicChoice, MusicReport, QualityReport,
    ResetReport, Revoked, StackEdit, StatusReport, Triggered, UpgradeMedia, UpgradeReport,
    VersionReport, WizardReport,
};
use lemonfiber_core::reconfigure::Stance;
use lemonfiber_core::seed::{
    Assessment, Report as SeedReport, Severity as SeedSeverity, State as SeedState, Wiring,
};
use lemonfiber_core::stored::{stored, Left, Removal};
use lemonfiber_core::walkthrough::{Shape, State as WalkState};
use lemonfiber_core::wizard::{Phase, Step};

use lemonfiber_core::config::paths::Paths;
use std::path::Path;

use super::{
    complain, exit_code, no_config_home, settled, shown, success, FAILURE, NEVER_SETTLED, USAGE,
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

fn a_wizard() -> WizardReport {
    WizardReport {
        offered: true,
        phase: Phase::InProgress,
        at: Step::DataLocation,
        asks: true,
        unanswered: Vec::new(),
        ready_for_review: false,
        plan: Vec::new(),
        written: Vec::new(),
        proof: None,
    }
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
    assert_eq!(
        coded(lemonfiber_core::error::codes::stack::STACK_INVALID),
        VALIDATION
    );
    // A file that will not parse and a name this build does not know are both
    // things the operator wrote, and a script that has to fix its own input
    // learns nothing from the same code it gets for a service being down.
    assert_eq!(
        coded(lemonfiber_core::error::codes::stack::STACK_MALFORMED),
        VALIDATION
    );
    assert_eq!(
        coded(lemonfiber_core::error::codes::stack::STACK_UNRECOGNISED),
        VALIDATION
    );
    assert_eq!(
        coded(lemonfiber_core::error::codes::config::CONFIG_UNREADABLE),
        VALIDATION
    );
    assert_eq!(
        coded(lemonfiber_core::error::codes::docker::ENGINE_UNREACHABLE),
        super::PREFLIGHT
    );
    assert_eq!(
        coded(lemonfiber_core::error::codes::life::NEVER_SETTLED),
        NEVER_SETTLED
    );
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
fn a_start_that_compose_did_not_carry_out_is_not_a_success() {
    // Waiting for services to become usable happens only where Compose exited
    // zero, so a failed start raises no problem at all. Without the status as the
    // verdict, a script could not tell a stack that came up from one that never
    // started.
    assert_eq!(
        shown(settled(&Outcome::Lifecycle(lifecycle(Some(0))))),
        success()
    );
    for status in [Some(1), Some(137), None] {
        assert_ne!(
            shown(settled(&Outcome::Lifecycle(lifecycle(status)))),
            success(),
            "{status:?}"
        );
    }
}

#[test]
fn an_apply_that_stopped_part_way_is_something_to_act_on_rather_than_a_success() {
    // Read back out of the progress file, `Applying` can only mean a previous run
    // died mid-write: what is written is written and what is not is not, and a
    // script that read success would go on against a machine that is in neither
    // state. Every other phase is the command doing what it was asked.
    let mut half_written = a_wizard();
    half_written.phase = Phase::Applying;
    assert_eq!(
        shown(settled(&Outcome::Wizard(half_written))),
        shown(std::process::ExitCode::from(VALIDATION))
    );

    for phase in [Phase::InProgress, Phase::Reviewing, Phase::Applied] {
        let mut standing = a_wizard();
        standing.phase = phase;
        assert_eq!(
            shown(settled(&Outcome::Wizard(standing))),
            success(),
            "{phase:?}"
        );
    }
}

#[test]
fn a_rehearsal_ran_nothing_and_so_failed_at_nothing() {
    // A rehearsal carries no status because it spawned no process, which is the
    // one case where an absent status is not a run that was signalled. Reading it
    // as a failure would make `--dry-run` unusable from a script — the very thing
    // the flag exists for.
    let mut rehearsed = lifecycle(None);
    rehearsed.rehearsed = true;
    assert_eq!(shown(settled(&Outcome::Lifecycle(rehearsed))), success());
}

#[test]
fn a_held_quality_choice_is_a_validation_result_rather_than_a_failure() {
    // Held means the operator has to say so explicitly, which is something they
    // can act on rather than something that went wrong.
    let held = QualityReport {
        choices: Vec::new(),
        music: None,
        customised: false,
        overwritten: None,
        disposition: Disposition::Held,
    };
    assert_ne!(format!("{:?}", settled(&Outcome::Quality(held))), success());
    let shown = QualityReport {
        choices: Vec::new(),
        music: None,
        customised: false,
        overwritten: None,
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

/// A removal that would take somebody, as it stands before it is confirmed.
fn removal(revoked: lemonfiber_core::model::Revoked) -> lemonfiber_core::model::HouseholdRemoval {
    lemonfiber_core::model::HouseholdRemoval {
        name: "ana".to_owned(),
        confirmed: !matches!(revoked, Revoked::Nothing),
        requests: 1,
        asks_through_the_request_service: true,
        revoked,
        findings: Vec::new(),
    }
}

mod changes;
mod questions;
