//! What the stack is doing, and what a command did to it.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.

use lemonfiber_core::docker::{Condition, Service, State};
use lemonfiber_core::model::{
    Envelope, LifecycleReport, ResetReport, StatusReport, SupervisionReport,
};
use lemonfiber_core::plural::s;
use lemonfiber_core::stack::closure::{Plan, Protocol};

use super::{Lines, UNRENDERABLE};

/// What a full reset did, or — until confirmed — would do: the edits it reverts to
/// lemonfiber's own state, named with what is lost, so nothing is discarded unseen.
pub(super) fn reset(report: &ResetReport) -> Lines {
    let mut lines = Lines::default();
    if report.reverted.is_empty() && report.reverted_connections.is_empty() {
        lines.put("Nothing to reset — the stack is already lemonfiber's own.");
        return lines;
    }
    let count = report.reverted.len() + report.reverted_connections.len();
    if report.confirmed {
        lines.put(format!("Reverted {count} change(s) to lemonfiber's state:"));
    } else {
        lines.put(format!(
            "A reset would revert these {count} change(s) to lemonfiber's state — run again \
             with --confirm to do it:"
        ));
    }
    // The service connections whose category drifted, named as they read in a seed report.
    for connection in &report.reverted_connections {
        lines.put(format!("  · {connection}"));
    }
    // The hand-edited stack files, each with the diff of what is lost.
    for edit in &report.reverted {
        lines.spaced(format!("  {}", edit.path));
        lines.block(&edit.diff);
    }
    lines
}

/// What naming a set of forms comes to, in the words to say before acting on it.
///
/// The services rather than the profiles, because a profile is an implementation
/// detail the operator was never shown and a service is the name they will see in
/// every other report — and the count, so a form that quietly grew is visible as a
/// number before it is a screenful.
pub(super) fn preview(plan: &Plan) -> Lines {
    let mut lines = Lines::default();
    let count = plan.services.len();
    // Two forms are a plural subject and take a plural verb. Naming several at
    // once is ordinary — `up full proxy` is the documented way to compose them —
    // so this is a sentence an operator reads, not a corner.
    let starts = if plan.forms.len() == 1 {
        "starts"
    } else {
        "start"
    };
    lines.put(format!(
        "{} {starts} {count} service{}: {}",
        plan.forms.join(" and "),
        s(count),
        plan.services.join(", ")
    ));
    lines.extend(left_out(plan));
    lines
}

/// What the configuration left out of a closure, and what each one wanted.
///
/// Said whenever a plan is, and that is the point: an operator seeing eleven
/// services where they expected fourteen is owed the reason at the moment they
/// notice, not in a diagnostic they have to think to run.
fn left_out(plan: &Plan) -> Lines {
    let mut lines = Lines::default();
    for out in &plan.dropped {
        lines.put(format!(
            "left out: {} — {}",
            out.profile,
            wanting(out.needs)
        ));
    }
    lines
}

/// The provider a dropped profile could not run without, as a sentence.
fn wanting(needs: Protocol) -> &'static str {
    match needs {
        Protocol::Usenet => "no Usenet provider is configured",
        Protocol::Torrent => "no VPN and torrent client are configured",
    }
}

/// What a lifecycle command did, or would have done.
pub(super) fn lifecycle(report: &LifecycleReport) -> Lines {
    let mut lines = Lines::default();
    if report.rehearsed {
        lines.put("would run:");
        lines.put(format!("  {}", report.command.join(" ")));
    }
    let profiles: Vec<&str> = report.plan.profiles.iter().map(String::as_str).collect();
    lines.put(format!("{}: {}", report.action, profiles.join(", ")));

    // Saying what was left out, and that it was deliberate, before the operator
    // goes looking for a service that was never going to start.
    lines.extend(left_out(&report.plan));

    if let Some(condition) = report.condition {
        lines.spaced(describe(condition));
        lines.extend(show(&report.services));
    }

    // Stack files the operator edited, kept as they set them rather than overwritten
    // on this run. Named with the change an upgrade would make, so the operator can
    // see what they are holding back before deciding to take it or keep theirs.
    for edit in &report.stack_edits {
        lines.spaced(format!(
            "kept {} as it is on disk — not overwritten by this version",
            edit.path
        ));
        lines.block(&edit.diff);
    }
    lines
}

/// What each service is doing.
pub(super) fn status(report: &StatusReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(describe(report.condition));
    lines.extend(show(&report.services));
    lines
}

/// A condition, as a sentence rather than as a word.
pub(super) fn describe(condition: Condition) -> &'static str {
    match condition {
        Condition::Inactive => "nothing is running",
        Condition::Degraded => "running, and something needs attention",
        Condition::Partial => "partly up",
        Condition::Active => "everything is up",
    }
}

/// What each service is doing, one per line.
pub(super) fn show(services: &[Service]) -> Lines {
    let mut lines = Lines::default();
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
        lines.put(format!("  {:<14} {state:<14} {}", service.id, service.name));
    }
    lines
}

/// The lines a finished watch renders to.
pub(super) fn watch_lines(report: &SupervisionReport, json: bool) -> Lines {
    let mut lines = Lines::default();
    if json {
        lines.put(
            Envelope::new("watch", report.clone())
                .to_json()
                .unwrap_or(UNRENDERABLE.to_owned()),
        );
        return lines;
    }

    lines.put(format!("the watch ended: {}", report.reason));
    if report.stopped {
        lines.put(format!("stopped: {}", report.forms.join(", ")));
    } else {
        lines.put(format!(
            "could not stop {} — check the services by hand",
            report.forms.join(", ")
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::fixtures::*;
    use lemonfiber_core::docker::{Condition, State};
    use lemonfiber_core::model::{
        LifecycleReport, ResetReport, StackEdit, StatusReport, SupervisionReport,
    };
    use lemonfiber_core::stack::closure::Dropped;

    #[test]
    fn a_reset_names_every_change_it_would_revert() {
        let report = ResetReport {
            reverted: vec![StackEdit {
                path: "compose.yml".to_owned(),
                diff: "-yours\n+ours\n".to_owned(),
            }],
            reverted_connections: vec!["sonarr → sabnzbd".to_owned()],
            confirmed: false,
        };
        let text = reset(&report).text();
        assert!(text.contains("would revert these 2 change(s)"));
        assert!(text.contains("· sonarr → sabnzbd"));
        assert!(text.contains("compose.yml"));
        assert!(text.contains("-yours"));
        // Confirmed, it reports what it did rather than what it would do.
        let done = ResetReport {
            confirmed: true,
            ..report
        };
        assert!(reset(&done).text().contains("Reverted 2 change(s)"));
        // Nothing to revert is not an empty list.
        let clean = ResetReport {
            reverted: Vec::new(),
            reverted_connections: Vec::new(),
            confirmed: false,
        };
        assert!(reset(&clean).text().contains("already lemonfiber's own"));
    }

    #[test]
    fn a_lifecycle_report_names_the_command_the_drops_and_the_edits_it_kept() {
        let report = LifecycleReport {
            command: vec!["docker".to_owned(), "compose".to_owned()],
            rehearsed: true,
            services: vec![service("sonarr", State::Healthy, None)],
            condition: Some(Condition::Active),
            stack_edits: vec![StackEdit {
                path: "compose.yml".to_owned(),
                diff: "-a\n+b\n".to_owned(),
            }],
            ..a_lifecycle(
                "up",
                a_plan(
                    "media",
                    vec![Dropped {
                        profile: "usenet".to_owned(),
                        needs: Protocol::Usenet,
                    }],
                ),
            )
        };
        let text = lifecycle(&report).text();
        assert!(text.contains("would run:"));
        assert!(text.contains("docker compose"));
        assert!(text.contains("up: media"));
        assert!(text.contains("left out: usenet — no Usenet provider is configured"));
        assert!(text.contains("everything is up"));
        assert!(text.contains("sonarr"));
        assert!(text.contains("kept compose.yml as it is on disk"));
        assert!(text.contains("-a"));
    }

    #[test]
    fn a_run_that_was_not_rehearsed_and_reports_no_condition_says_only_what_it_did() {
        let report = LifecycleReport {
            status: Some(0),
            ..a_lifecycle("down", a_plan("media", Vec::new()))
        };
        let text = lifecycle(&report).text();
        assert_eq!(text, "down: media");
    }

    #[test]
    fn a_plan_says_what_starts_and_what_the_configuration_left_out() {
        let text = preview(&a_plan(
            "tv",
            vec![Dropped {
                profile: "torrent".to_owned(),
                needs: Protocol::Torrent,
            }],
        ))
        .text();
        assert!(
            text.contains("tv starts 1 service: sonarr"),
            "the services, counted, in the operator's own words: {text}"
        );
        assert!(text.contains("left out: torrent — no VPN and torrent client are configured"));
    }

    #[test]
    fn two_forms_named_together_take_a_plural_verb() {
        let composed = Plan {
            forms: vec!["full".to_owned(), "proxy".to_owned()],
            ..a_plan("tv", Vec::new())
        };
        let text = preview(&composed).text();
        assert!(
            text.starts_with("full and proxy start 1 service"),
            "a plural subject takes a plural verb: {text}"
        );
    }

    #[test]
    fn a_plan_that_starts_several_services_counts_them_as_several() {
        let several = Plan {
            services: vec!["sonarr".to_owned(), "bazarr".to_owned()],
            ..a_plan("tv", Vec::new())
        };
        let text = preview(&several).text();
        assert!(text.contains("starts 2 services: sonarr, bazarr"), "{text}");
        assert!(
            !text.contains("left out"),
            "nothing was left out, so nothing is said about it: {text}"
        );
    }

    #[test]
    fn every_condition_reads_as_a_sentence() {
        for condition in [
            Condition::Inactive,
            Condition::Degraded,
            Condition::Partial,
            Condition::Active,
        ] {
            assert!(!describe(condition).is_empty());
        }
        assert_eq!(describe(Condition::Inactive), "nothing is running");
    }

    #[test]
    fn every_service_state_reads_as_a_word_and_a_failure_carries_its_code() {
        let services = vec![
            service("a", State::Absent, None),
            service("b", State::Stopped, None),
            service("c", State::Starting, None),
            service("d", State::Running, None),
            service("e", State::Healthy, None),
            service("f", State::Unhealthy, None),
            service("g", State::CrashLooping, None),
            service("h", State::HostManaged, None),
            service("i", State::Failed, Some(137)),
            service("j", State::Failed, None),
        ];
        let text = show(&services).text();
        for word in [
            "absent",
            "stopped",
            "starting",
            "running",
            "healthy",
            "unhealthy",
            "crash-looping",
            "host-managed",
        ] {
            assert!(text.contains(word), "missing {word}");
        }
        // The exit code is the whole reason a failure is not simply "stopped".
        assert!(text.contains("failed (137)"));
        assert!(text.contains("failed         j service"));
    }

    #[test]
    fn a_status_report_leads_with_the_condition() {
        let report = StatusReport {
            forms: vec!["media".to_owned()],
            condition: Condition::Degraded,
            services: vec![service("sonarr", State::Unhealthy, None)],
        };
        let text = status(&report).text();
        assert!(text.starts_with("running, and something needs attention"));
        assert!(text.contains("unhealthy"));
    }

    #[test]
    fn a_watch_reports_whether_it_stopped_what_it_guarded() {
        assert!(watch_lines(&a_watch(), false)
            .text()
            .contains("stopped: media"));
        let stranded = SupervisionReport {
            stopped: false,
            ..a_watch()
        };
        assert!(watch_lines(&stranded, false)
            .text()
            .contains("could not stop media"));
        // As JSON it is one envelope, not prose.
        let json = watch_lines(&a_watch(), true).text();
        assert!(json.contains(r#""kind":"watch""#));
    }
}
