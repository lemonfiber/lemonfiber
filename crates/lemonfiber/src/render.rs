//! The surface's rendering, kept apart from the CLI wiring and orchestration.
//!
//! `main` decides what to run and hands the outcome here; this module decides
//! only how it reads. One renderer per answer, for a person or for a script,
//! with nothing about parsing input or dispatching commands mixed in — so the
//! shape of an operator's report and the shape of the command line stay two
//! separate things to change.

use lemonfiber_core::app::Outcome;
use lemonfiber_core::docker::{Condition, Service, State};
use lemonfiber_core::doctor::{Overall, Verdict};
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{
    ConfigReport, DoctorReport, Envelope, LifecycleReport, StatusReport, SupervisionReport,
    VersionReport,
};
use lemonfiber_core::seed::{
    Assessment as SeedAssessment, Report as SeedReport, State as SeedState,
};
use lemonfiber_core::PRODUCT;

/// Render an outcome, for a person or for a script.
///
/// One renderer per answer, rather than one function that knows all four. They
/// have nothing in common beyond arriving here: what a version report owes an
/// operator and what a lifecycle report owes them are different questions, and
/// a single body deciding both reads as one thing with four moods.
pub(crate) fn render(outcome: &Outcome, json: bool) {
    if json {
        machine_readable(outcome);
        return;
    }

    match outcome {
        Outcome::Version(report) => versions(report),
        Outcome::Config(report) => settings(report),
        Outcome::Lifecycle(report) => lifecycle(report),
        Outcome::Status(report) => status(report),
        Outcome::Doctor(report) => diagnosis(report),
        Outcome::Seed(report) => seeding(report),
    }
}

/// What seeding wired, connection by connection, with what a re-run still owes
/// named last so it is the thing the operator is left looking at.
pub(crate) fn seeding(report: &SeedReport) {
    for wiring in &report.wirings {
        match &wiring.state {
            SeedState::Wired => println!("  ✓ {}   wired", wiring.connection),
            SeedState::AlreadyWired => println!("  ✓ {}   already wired", wiring.connection),
            SeedState::Drifted => println!("  · {}   left as you set it", wiring.connection),
            SeedState::Adopted => println!("  ✓ {}   yours, adopted", wiring.connection),
            SeedState::Unmanaged => println!(
                "  · {}   found already set — yours, left as is (run `{PRODUCT} adopt` to keep it)",
                wiring.connection
            ),
            SeedState::Stale => println!(
                "  · {}   yours for now — a newer default is not yet applied",
                wiring.connection
            ),
            SeedState::Conflicted { yours, ours } => {
                println!(
                    "  ✗ {}   conflict — both you and the default changed it",
                    wiring.connection
                );
                match yours {
                    Some(yours) => println!(
                        "      you set “{yours}”, the default is now “{ours}” — left as you set it"
                    ),
                    None => println!(
                        "      you cleared it, the default is now “{ours}” — left as you set it"
                    ),
                }
            }
            SeedState::Skipped { reason } => {
                println!("  ? {}   skipped", wiring.connection);
                println!("      {reason}");
            }
            SeedState::Failed { detail } => {
                println!("  ✗ {}   {detail}", wiring.connection);
            }
            SeedState::Refused { reason } => {
                println!("  ✗ {}   refused", wiring.connection);
                println!("      {reason}");
            }
        }
    }
    let outstanding = report.outstanding();
    let blocked = report.blocked();
    if outstanding.is_empty() {
        println!("\nEverything is wired.");
    } else if blocked.is_empty() {
        println!(
            "\n{} left to wire — run seed again once ready.",
            outstanding.len()
        );
    } else if blocked.len() == outstanding.len() {
        println!(
            "\n{} to resolve — settle the conflict, then seed again.",
            blocked.len()
        );
    } else {
        println!(
            "\n{} left: {} to wire once ready, {} to resolve — settle the conflict first.",
            outstanding.len(),
            outstanding.len() - blocked.len(),
            blocked.len(),
        );
    }
    if matches!(report.assessment, SeedAssessment::Unassessable) {
        println!(
            "\nThe record of what lemonfiber last wrote could not be read, so drift \
             could not be assessed this run. Run `lemonfiber adopt` to re-baseline \
             from the current state."
        );
    }
}

/// What the diagnostic checks found, finding by finding.
///
/// Each finding leads with a mark that reads at a glance and the plain evidence
/// behind it; a non-passing one carries the reason and what to do, because a
/// finding without a remedy is a fault report rather than a diagnosis.
pub(crate) fn diagnosis(report: &DoctorReport) {
    for finding in &report.findings {
        match &finding.verdict {
            Verdict::Pass { note } => match note {
                Some(note) => println!("  ✓ {}   {note}", finding.title),
                None => println!("  ✓ {}", finding.title),
            },
            Verdict::Warn(problem) => {
                println!("  ! {}   {}", finding.title, problem.summary);
                remedies(problem);
            }
            Verdict::Fail(problem) => {
                println!("  ✗ {}   {}", finding.title, problem.summary);
                remedies(problem);
            }
            Verdict::Unverified { reason, remedy } => {
                println!("  ? {}   UNVERIFIED", finding.title);
                println!("      {reason}");
                println!("      → {}", remedy.action);
                if let Some(detail) = &remedy.detail {
                    println!("        {detail}");
                }
            }
            Verdict::Skipped { reason } => {
                println!("  – {}   skipped: {reason}", finding.title);
            }
        }
    }

    println!("\n{}", overall(report.overall));
}

/// The problem's meaning and remedies, indented under a finding.
pub(crate) fn remedies(problem: &Problem) {
    println!("      {}", problem.meaning);
    for remedy in &problem.remedies {
        println!("      → {}", remedy.action);
        if let Some(detail) = &remedy.detail {
            println!("        {detail}");
        }
    }
}

/// The one-line verdict a diagnosis amounts to.
pub(crate) fn overall(overall: Overall) -> &'static str {
    match overall {
        Overall::Healthy => "healthy — everything checked passed",
        Overall::Degraded => "degraded — working, with warnings",
        Overall::Broken => "broken — something needs attention",
        Overall::Unknown => "unknown — health could not be established",
    }
}

/// The same answer, for something that will parse it.
pub(crate) fn machine_readable(outcome: &Outcome) {
    match outcome.clone().envelope().to_json() {
        Some(text) => println!("{text}"),
        None => eprintln!("this outcome could not be rendered as JSON"),
    }
}

/// What versions are in play.
pub(crate) fn versions(report: &VersionReport) {
    println!("{PRODUCT} {}", report.binary);
    println!("stack {}", report.stack);
    println!("manifest schema {:?}", report.supported_schema);
    match &report.compose {
        Some(version) => println!("compose {version}"),
        None => println!("compose not reachable"),
    }
}

/// What the operator has configured.
pub(crate) fn settings(report: &ConfigReport) {
    for setting in &report.settings {
        println!("{}={}", setting.key, setting.value);
    }
    if report.changed {
        // A rehearsal reports what it would do, so it must not claim it saved.
        println!(
            "{}",
            if report.rehearsed {
                "would save"
            } else {
                "saved"
            }
        );
    }
}

/// What a lifecycle command did, or would have done.
pub(crate) fn lifecycle(report: &LifecycleReport) {
    if report.rehearsed {
        println!("would run:\n  {}", report.command.join(" "));
    }
    println!("{}: {}", report.action, report.profiles.join(", "));

    // Saying what was left out, and that it was deliberate, before the operator
    // goes looking for a service that was never going to start.
    if !report.dropped.is_empty() {
        println!(
            "left out (no provider configured): {}",
            report.dropped.join(", ")
        );
    }

    if let Some(condition) = report.condition {
        println!("\n{}", describe(condition));
        show(&report.services);
    }

    // Stack files the operator edited, kept as they set them rather than overwritten
    // on this run. Named with the change an upgrade would make, so the operator can
    // see what they are holding back before deciding to take it or keep theirs.
    for edit in &report.stack_edits {
        println!(
            "\nkept {} as it is on disk — not overwritten by this version",
            edit.path
        );
        if !edit.diff.is_empty() {
            print!("{}", edit.diff);
        }
    }
}

/// What each service is doing.
pub(crate) fn status(report: &StatusReport) {
    println!("{}", describe(report.condition));
    show(&report.services);
}

/// A condition, as a sentence rather than as a word.
pub(crate) fn describe(condition: Condition) -> &'static str {
    match condition {
        Condition::Inactive => "nothing is running",
        Condition::Degraded => "running, and something needs attention",
        Condition::Partial => "partly up",
        Condition::Active => "everything is up",
    }
}

/// What each service is doing, one per line.
pub(crate) fn show(services: &[Service]) {
    for service in services {
        let state = match service.state {
            State::Absent => "absent".to_owned(),
            State::Stopped => "stopped".to_owned(),
            State::Starting => "starting".to_owned(),
            State::Running => "running".to_owned(),
            State::Healthy => "healthy".to_owned(),
            State::Unhealthy => "unhealthy".to_owned(),
            State::CrashLooping => "crash-looping".to_owned(),
            State::HostManaged => "host-managed".to_owned(),
            // The code is the whole reason this is not simply "stopped", so it
            // is shown rather than left for the operator to go and find.
            State::Failed => match service.exit {
                Some(code) => format!("failed ({code})"),
                None => "failed".to_owned(),
            },
        };
        println!("  {:<14} {:<14} {}", service.id, state, service.name);
    }
}

/// What a watch did once its location was lost.
pub(crate) fn watched(report: &SupervisionReport, json: bool) {
    if json {
        match Envelope::new("watch", report.clone()).to_json() {
            Some(text) => println!("{text}"),
            None => eprintln!("this report could not be rendered as JSON"),
        }
        return;
    }

    println!("the watch ended: {}", report.reason);
    if report.stopped {
        println!("stopped: {}", report.forms.join(", "));
    } else {
        println!(
            "could not stop {} — check the services by hand",
            report.forms.join(", ")
        );
    }
}
