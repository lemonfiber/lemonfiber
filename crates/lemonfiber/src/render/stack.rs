//! What the stack is doing, and what a command did to it.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.

use lemonfiber_core::docker::{Condition, Service, State, Undeclared};
use lemonfiber_core::model::{
    ConflictReport, LifecycleReport, ResetReport, StatusReport, SupervisionReport, Switched,
    UnsupportedReport, Vigil,
};
use lemonfiber_core::plural::s;
use lemonfiber_core::stack::closure::{Plan, Protocol};
use lemonfiber_core::text::plain;

use super::Lines;

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
    affects(plan, Doing::Starting)
}

/// Which way round an operation is about to move things.
///
/// Only the verb differs: the services a form holds are the same ones whether they
/// are about to go up or come down, so an operator reads one sentence in both cases
/// and the closure is resolved once by the same code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Doing {
    /// About to bring them up.
    Starting,
    /// About to take them down.
    Stopping,
}

impl Doing {
    /// The verb, agreeing with however many forms were named.
    const fn verb(self, several: bool) -> &'static str {
        match (self, several) {
            (Self::Starting, false) => "starts",
            (Self::Starting, true) => "start",
            (Self::Stopping, false) => "stops",
            (Self::Stopping, true) => "stop",
        }
    }
}

/// What naming these forms will affect, said before anything is done to them.
///
/// The operator is told which services an operation reaches *before* it reaches them,
/// so a command that touches more than they meant is something they see coming rather
/// than something they read about afterwards.
pub(crate) fn affects(plan: &Plan, doing: Doing) -> Lines {
    let mut lines = Lines::default();
    let count = plan.services.len();
    // Two forms are a plural subject and take a plural verb. Naming several at
    // once is ordinary — `up full proxy` is the documented way to compose them —
    // so this is a sentence an operator reads, not a corner.
    let verb = doing.verb(plan.forms.len() != 1);
    lines.put(format!(
        "{} {verb} {count} service{}: {}",
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

/// What narrowing the active set moved, a line per direction.
///
/// A direction that moved nothing is left unsaid rather than reported as none: on a
/// stack that was down, "stopped: nothing" and "kept: nothing" are two lines that
/// tell the operator only that they read the report.
fn moved(switched: &Switched) -> Lines {
    let mut lines = Lines::default();

    // Nothing moved is an answer rather than an absence. An operator who asked for a
    // shape the stack is already in should be told so, not left to infer it from a
    // report that says only which profiles were named. What is still running is listed
    // underneath, because "already in that shape" is worth more with the shape beside
    // it.
    if switched.stopped.is_empty() && switched.started.is_empty() {
        lines.put("already in that shape — nothing was stopped or started");
    }

    for (what, services) in [
        ("stopped", &switched.stopped),
        ("started", &switched.started),
        ("kept running", &switched.kept),
    ] {
        if !services.is_empty() {
            lines.put(format!("{what}: {}", services.join(", ")));
        }
    }
    lines
}

/// What to say about a lifecycle run Compose did not carry out.
///
/// The code is named because it is the operator's way into Compose's own account of
/// what went wrong, and because it is the number a script's caller will branch on. A
/// run carrying no code at all was signalled rather than ended — a different thing to
/// have happened, and a different thing to go looking into.
fn unfinished(report: &LifecycleReport) -> String {
    match report.status {
        Some(code) => format!("{} did not finish — Compose exited {code}", report.action),
        None => format!(
            "{} did not finish — Compose was stopped before it ended",
            report.action
        ),
    }
}

/// What a lifecycle command did, or would have done.
pub(crate) fn lifecycle(report: &LifecycleReport) -> Lines {
    let mut lines = Lines::default();
    // A start that declined to start anything has one thing to say, and the plan
    // underneath it is the plan it did not run. Printing that first would read as an
    // account of what happened, which is the opposite of what this report is.
    if let Some(held) = &report.held {
        lines.put(format!("{}: nothing was started — {held}", report.action));
        return lines;
    }
    if report.rehearsed {
        lines.put("would run:");
        // Both invocations, in the order they would run. A switch that stops
        // something runs two commands, and printing one of them would make the
        // rehearsal a smaller claim than the thing it is rehearsing.
        if let Some(stopping) = report
            .switched
            .as_ref()
            .and_then(|switched| switched.stop_command.as_ref())
        {
            lines.put(format!("  {}", stopping.join(" ")));
        }
        lines.put(format!("  {}", report.command.join(" ")));
    }
    let profiles: Vec<&str> = report.plan.profiles.iter().map(String::as_str).collect();
    lines.put(format!("{}: {}", report.action, profiles.join(", ")));

    // A run Compose did not carry out says so here rather than leaving the operator to
    // infer it from the settled services that are missing below. The exit status says
    // the same thing to a script, and the two are decided from the same field, so a
    // screen and a caller cannot come away with different accounts of one run.
    if !report.rehearsed && report.status != Some(0) {
        lines.put(unfinished(report));
    }

    // What narrowing moved. The kept list is the point of the verb — it is the
    // promise that a download in flight was not interrupted — so it is said even
    // though, by definition, nothing happened to it.
    if let Some(switched) = &report.switched {
        lines.extend(moved(switched));
    }

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

    lines.extend(clashes(&report.port_conflicts));
    lines
}

/// Host ports something else on this machine already answers on.
///
/// Said whether the start went ahead or not, and said as a fact rather than as a
/// refusal: sharing a port on purpose is the operator's business. What it buys is
/// that an operator meeting a bind failure is told who is holding the port instead
/// of going and finding out — and, on a rehearsal, is told before anything runs.
///
/// The holder is named rather than described in the opening line, because it can be
/// another Compose project or it can be a program the operator started themselves,
/// and an opening line that named one of those would be wrong about the other.
fn clashes(conflicts: &[ConflictReport]) -> Lines {
    let mut lines = Lines::default();
    if conflicts.is_empty() {
        return lines;
    }
    lines.spaced(
        "Something on this machine already answers on these ports, so these services \
         will not be able to bind:",
    );
    for clash in conflicts {
        lines.put(format!(
            "  {} wants {}, held by {}",
            clash.wanted_by, clash.port, clash.held_by
        ));
    }
    lines.spaced(
        "Move them with `lemonfiber migrate beside`, which writes a Compose file of \
         ports nothing else is using, or change the ports yourself.",
    );
    lines
}

/// What each service is doing.
pub(super) fn status(report: &StatusReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(describe(report.condition));
    lines.extend(show(&report.services));
    lines.extend(strangers(&report.undeclared));
    lines.extend(unsupported(&report.unsupported));
    lines
}

/// The containers running under this project that the stack never declared.
///
/// Shown rather than left out, and shown apart rather than mixed in. An operator
/// looking at what is running is entitled to see everything running under the name
/// lemonfiber started things under, including the things lemonfiber did not start —
/// a listing that quietly dropped them would be an answer about the stack description
/// wearing the clothes of an answer about the machine. Apart, because the one thing
/// said about each of them is that nothing here knows what it is, and putting that in
/// the same column as nineteen real descriptions would read as a service whose
/// description had gone missing.
///
/// Silent where there are none, which is every ordinary stack.
fn strangers(undeclared: &[Undeclared]) -> Lines {
    let mut lines = Lines::default();
    if undeclared.is_empty() {
        return lines;
    }
    lines.spaced(format!(
        "{} container{} running under this project that the stack does not declare:",
        undeclared.len(),
        s(undeclared.len())
    ));
    for one in undeclared {
        // The name is the engine's rather than the manifest's, so it is made plain
        // before it is padded rather than after: the width has to be counted on what
        // will actually be drawn, or a container named with control characters pushes
        // every column after it out of true.
        lines.put(format!(
            "  {:<14} {:<14} {}",
            plain(&one.id),
            worded(one.state),
            one.describes
        ));
    }
    lines
}

/// The services lemonfiber runs and cannot speak to, under the states of the ones it
/// can.
///
/// Below rather than beside, and absent entirely on a stack where there are none —
/// which is every stack this build ships. What it says is deliberately not a fault:
/// these services are up, and every generic thing works on them. What is missing is
/// the part that needs to know which service it is talking to, and the operator who
/// wrote the declaration is the only person who can complete it.
fn unsupported(reports: &[UnsupportedReport]) -> Lines {
    let mut lines = Lines::default();
    if reports.is_empty() {
        return lines;
    }
    lines.spaced(
        "These run and are yours to start, stop and watch like any other. What \
         lemonfiber cannot do is anything that needs to know what they are:",
    );
    for report in reports {
        lines.put(format!("  {} — {}", report.what, report.because));
    }
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

/// What one service is doing, in the word an operator reads.
///
/// One spelling, because the screen that offers a service to act on says what it is
/// doing beside the name — and two spellings of `crash-looping` is two accounts of
/// the same container.
pub(crate) fn doing(service: &Service) -> String {
    match service.state {
        // The code is the whole reason this is not simply "stopped", so it
        // is shown rather than left for the operator to go and find.
        State::Failed => match service.exit {
            Some(code) => format!("failed ({code})"),
            None => "failed".to_owned(),
        },
        state => worded(state).to_owned(),
    }
}

/// What one state is called, with nothing of the container it belongs to.
///
/// Split out from [`doing`] because a container the stack never declared has a state
/// and no service behind it to ask for an exit code, and two spellings of
/// `crash-looping` on one screen is two accounts of the same thing.
const fn worded(state: State) -> &'static str {
    match state {
        State::Absent => "absent",
        State::Stopped => "stopped",
        State::Starting => "starting",
        State::Running => "running",
        State::Healthy => "healthy",
        State::Unhealthy => "unhealthy",
        State::CrashLooping => "crash-looping",
        State::HostManaged => "host-managed",
        State::Failed => "failed",
    }
}

/// What each service is doing, one per line, with what it is for on the end of it.
///
/// The description rides the line the operator is already reading rather than waiting
/// in a command they would have to think to run — which is the whole of what having it
/// available where a service is referenced comes to. Last on the line, because the
/// state is what somebody scanning the list is scanning for and a sentence in front of
/// it would push the column that matters off the left of their attention.
///
/// A service the stack describes with nothing gets no dash rather than an empty one:
/// the manifest requires a description of every service, so this is the fork a fork's
/// own stack takes, and a trailing punctuation mark with nothing after it reads as a
/// rendering that broke.
pub(super) fn show(services: &[Service]) -> Lines {
    let mut lines = Lines::default();
    for service in services {
        let state = doing(service);
        // The id and the name are made plain before they are padded, for the reason
        // a log line's service is: both came from a stack description somebody can
        // edit, and a width counted on characters a terminal will swallow is a column
        // that lands somewhere else than where it was measured.
        lines.put(format!(
            "  {:<14} {state:<14} {}{}",
            plain(&service.id),
            plain(&service.name),
            what_for(service)
        ));
    }
    lines
}

/// What a service is for, as the tail of the line naming it.
fn what_for(service: &Service) -> String {
    if service.describes.trim().is_empty() {
        return String::new();
    }
    format!(" — {}", service.describes)
}

/// The lines a finished watch renders to.
///
/// For a person only. A machine-readable run renders the envelope the outcome
/// carries, the way every other answer does, so there is no second rendering here
/// that could describe the same ending differently.
pub(super) fn watch(report: &SupervisionReport) -> Lines {
    if let Some(would) = &report.would {
        return kept(would, &report.forms);
    }

    let mut lines = Lines::default();
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

/// The watch a rehearsal would keep, said the way a rehearsed lifecycle command says
/// what it would run: the argv first, because that is the sentence an operator is
/// checking, and the terms of the guard around it.
fn kept(would: &Vigil, forms: &[String]) -> Lines {
    let mut lines = Lines::default();
    lines.put("would run, the moment the data location went:");
    lines.put(format!("  {}", would.command.join(" ")));
    lines.spaced(format!(
        "watching {} every {}s, and stopping {}",
        would.root,
        would.every,
        naming(forms)
    ));
    lines.spaced("Nothing is being watched. Run it without --dry-run to keep the guard.");
    lines
}

/// The forms a watch would stop, or what naming none of them means.
///
/// Said rather than left as an empty list, because a line ending in nothing reads as a
/// guard that would stop nothing at all — which is the opposite of what naming no form
/// asks for.
fn naming(forms: &[String]) -> String {
    if forms.is_empty() {
        return "the whole stack".to_owned();
    }
    forms.join(", ")
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

    /// Told before the bind fails, and told as a fact rather than a refusal.
    #[test]
    fn a_port_something_else_holds_is_named_on_both_sides_before_the_start() {
        let report = LifecycleReport {
            rehearsed: true,
            port_conflicts: vec![ConflictReport {
                port: 8989,
                wanted_by: "sonarr".to_owned(),
                held_by: "somebody-elses/sonarr".to_owned(),
            }],
            ..a_lifecycle("up", a_plan("media", Vec::new()))
        };
        let text = lifecycle(&report).text();
        assert!(
            text.contains("sonarr wants 8989, held by somebody-elses/sonarr"),
            "{text}"
        );
        assert!(text.contains("migrate beside"), "{text}");
    }

    /// And a machine running one stack hears nothing about it.
    #[test]
    fn a_start_with_nothing_in_its_way_says_nothing_about_ports() {
        let report = a_lifecycle("up", a_plan("media", Vec::new()));
        assert!(!lifecycle(&report).text().contains("already answers on"));
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

    /// The half of the failed-start defect a person sees. The other half is the exit
    /// status, and both are read from the same field so a screen and a script cannot
    /// come away with different accounts of one run.
    #[test]
    fn a_run_compose_did_not_carry_out_says_so_rather_than_reading_as_a_success() {
        let failed = LifecycleReport {
            status: Some(1),
            ..a_lifecycle("up", a_plan("media", Vec::new()))
        };
        let said = lifecycle(&failed).text();
        assert!(
            said.contains("up did not finish — Compose exited 1"),
            "{said}"
        );

        // No status at all, on a run that was not a rehearsal, is a process that was
        // signalled rather than one that exited — there is no code to name, and saying
        // one anyway would invent a number Compose never produced.
        let signalled = LifecycleReport {
            status: None,
            ..a_lifecycle("up", a_plan("media", Vec::new()))
        };
        let stopped = lifecycle(&signalled).text();
        assert!(
            stopped.contains("Compose was stopped before it ended"),
            "{stopped}"
        );

        // A rehearsal spawned nothing, so it failed at nothing — saying it did not
        // finish would make `--dry-run` read as a broken command on every run.
        let rehearsed = LifecycleReport {
            status: None,
            rehearsed: true,
            ..a_lifecycle("up", a_plan("media", Vec::new()))
        };
        assert!(!lifecycle(&rehearsed).text().contains("did not finish"));
    }

    /// A start that declined to start anything says why, and says nothing else.
    ///
    /// The start a login makes declines for three reasons an operator would act on
    /// differently — the stack was stopped on purpose, autostart was never asked for,
    /// the machine is on its battery — and the plan carried underneath is the plan it
    /// did not run. Rendering the profiles, the services and the condition alongside
    /// would read as an account of what happened, and an operator skimming it would
    /// come away believing their stack came back. So the whole rendering is the one
    /// sentence, which is why this asserts the whole of the text rather than a
    /// fragment of it.
    #[test]
    fn a_start_that_declined_says_why_and_nothing_of_the_plan_it_did_not_run() {
        let declined = LifecycleReport {
            held: Some("the stack was stopped on purpose".to_owned()),
            ..a_lifecycle("boot", a_plan("media", Vec::new()))
        };

        assert_eq!(
            lifecycle(&declined).text(),
            "boot: nothing was started — the stack was stopped on purpose"
        );
    }

    #[test]
    fn a_switch_says_what_moved_in_each_direction() {
        let report = LifecycleReport {
            switched: Some(Switched {
                stopped: vec!["qbittorrent".to_owned()],
                started: vec!["sonarr".to_owned()],
                kept: vec!["jellyfin".to_owned()],
                stop_command: None,
            }),
            ..a_lifecycle("switch", a_plan("media", Vec::new()))
        };
        let text = lifecycle(&report).text();
        assert!(text.contains("stopped: qbittorrent"), "{text}");
        assert!(text.contains("started: sonarr"), "{text}");
        assert!(
            text.contains("kept running: jellyfin"),
            "the one that makes the verb worth having: {text}"
        );
    }

    /// A switch onto the shape the stack is already in moved nothing at all, and that
    /// is an answer. Reporting only the profiles would leave the operator to work out
    /// for themselves that nothing happened.
    #[test]
    fn a_switch_that_moved_nothing_says_so_and_still_shows_what_is_up() {
        let report = LifecycleReport {
            switched: Some(Switched {
                stopped: Vec::new(),
                started: Vec::new(),
                kept: vec!["jellyfin".to_owned(), "seerr".to_owned()],
                stop_command: None,
            }),
            ..a_lifecycle("switch", a_plan("media", Vec::new()))
        };

        let text = lifecycle(&report).text();
        assert!(
            text.contains("already in that shape"),
            "the no-op is stated rather than left to be inferred: {text}"
        );
        assert!(
            text.contains("kept running: jellyfin, seerr"),
            "and what is still up is worth more beside it than alone: {text}"
        );
    }

    /// A direction that moved nothing says nothing, rather than saying "none" three
    /// times to an operator who switched onto a stack that was down.
    #[test]
    fn a_switch_that_moved_nothing_in_a_direction_leaves_that_direction_unsaid() {
        let report = LifecycleReport {
            switched: Some(Switched {
                stopped: Vec::new(),
                started: vec!["sonarr".to_owned()],
                kept: Vec::new(),
                stop_command: None,
            }),
            ..a_lifecycle("switch", a_plan("media", Vec::new()))
        };
        let text = lifecycle(&report).text();
        assert_eq!(text, "switch: media\nstarted: sonarr");
    }

    /// A switch that stops something runs two commands, and a rehearsal that printed
    /// one of them would be a smaller claim than the thing it is rehearsing.
    #[test]
    fn a_rehearsed_switch_prints_both_invocations_in_the_order_they_would_run() {
        let report = LifecycleReport {
            rehearsed: true,
            command: vec!["docker".to_owned(), "compose".to_owned(), "up".to_owned()],
            switched: Some(Switched {
                stopped: vec!["qbittorrent".to_owned()],
                started: Vec::new(),
                kept: Vec::new(),
                stop_command: Some(vec![
                    "docker".to_owned(),
                    "compose".to_owned(),
                    "stop".to_owned(),
                ]),
            }),
            ..a_lifecycle("switch", a_plan("media", Vec::new()))
        };
        let text = lifecycle(&report).text();
        let stopping = text.find("compose stop");
        let starting = text.find("compose up");
        assert!(
            stopping.is_some() && starting.is_some() && stopping < starting,
            "the stop is printed first, because it runs first: {text}"
        );
    }

    /// The operator is told what an operation reaches before it reaches it, and the
    /// sentence says which way round. The services are the same either way — only the
    /// verb changes — so both directions resolve through the same code.
    #[test]
    fn what_an_operation_affects_is_said_in_the_direction_it_is_going() {
        let plan = a_plan("media", Vec::new());
        let starting = affects(&plan, Doing::Starting).text();
        let stopping = affects(&plan, Doing::Stopping).text();

        assert!(starting.contains(" starts "), "{starting}");
        assert!(stopping.contains(" stops "), "{stopping}");
        assert!(
            starting.contains("sonarr") && stopping.contains("sonarr"),
            "the same services either way: {starting} / {stopping}"
        );
    }

    /// Naming two forms at once is the documented way to compose them, so the verb has
    /// to agree with a plural subject rather than reading as a corner nobody hits.
    #[test]
    fn two_forms_named_at_once_take_a_plural_verb() {
        let mut plan = a_plan("media", Vec::new());
        plan.forms = vec!["tv".to_owned(), "movies".to_owned()];
        let said = affects(&plan, Doing::Stopping).text();
        assert!(said.contains("tv and movies stop "), "{said}");
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
            undeclared: Vec::new(),
            services: vec![service("sonarr", State::Unhealthy, None)],
            disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
            unsupported: Vec::new(),
        };
        let text = status(&report).text();
        assert!(text.starts_with("running, and something needs attention"));
        assert!(text.contains("unhealthy"));
    }

    /// What a service is for rides the line an operator is already reading, which is
    /// the whole of what having it available where a service is referenced comes to.
    #[test]
    fn a_service_is_listed_with_what_it_does_for_the_operator() {
        let text = show(&[service("sonarr", State::Healthy, None)]).text();

        assert!(text.contains("what sonarr is for"), "{text}");
        assert!(
            text.contains("sonarr service — what sonarr is for"),
            "and after the name rather than in front of the state: {text}"
        );
    }

    /// A stack that says nothing about a service gets no dangling dash, because a
    /// punctuation mark with nothing after it reads as a rendering that broke.
    #[test]
    fn a_service_the_stack_describes_with_nothing_is_listed_without_a_dash() {
        let quiet = Service {
            describes: String::new(),
            ..service("sonarr", State::Healthy, None)
        };
        let text = show(&[quiet]).text();

        assert!(!text.contains('—'), "{text}");
        assert!(text.contains("sonarr service"), "{text}");
    }

    /// A description is prose out of a file an operator can edit, and this is the one
    /// new place such prose reaches a terminal — so what a terminal obeys is taken out
    /// of it, and taken out before anything is measured.
    ///
    /// Two things are being held here rather than one. A control sequence in the
    /// middle of a description would clear the screen the listing was being drawn on;
    /// a newline in it would let one service's description draw a line that looked
    /// like the next service's row, which is how a stack description comes to hide a
    /// service that is down behind a service that is not.
    #[test]
    fn a_description_cannot_take_over_the_screen_or_forge_a_row() {
        let hostile = Service {
            describes: "fine\u{1b}[2J\nqbittorrent     healthy        all is well".to_owned(),
            ..service("sonarr", State::Healthy, None)
        };
        let text = show(&[hostile]).text();

        assert!(!text.contains('\u{1b}'), "{text:?}");
        assert_eq!(
            text.lines().count(),
            1,
            "one service is one row, whatever the description says: {text:?}"
        );
    }

    /// A container nobody declared is shown with an unknown description rather than
    /// dropped from the listing, and shown apart from the services — the one thing
    /// that can be said about it is that nothing here knows what it is.
    #[test]
    fn a_container_the_stack_never_declared_is_shown_with_an_unknown_description() {
        let report = StatusReport {
            forms: Vec::new(),
            condition: Condition::Inactive,
            undeclared: vec![Undeclared {
                id: "something-of-their-own".to_owned(),
                state: State::Running,
                describes: lemonfiber_core::docker::UNDESCRIBED.to_owned(),
            }],
            services: vec![service("sonarr", State::Absent, None)],
            disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
            unsupported: Vec::new(),
        };
        let text = status(&report).text();

        assert!(
            text.contains(
                "1 container running under this project that the stack does not \
                           declare:"
            ),
            "{text}"
        );
        assert!(text.contains("something-of-their-own"), "{text}");
        assert!(text.contains("not declared by this stack"), "{text}");
    }

    /// A stranger that failed is said to have failed, and nothing more.
    ///
    /// The word is the whole of the answer here: a declared service that failed is
    /// shown with the code it exited on, and there is no service behind a container
    /// the manifest never named to ask one of. Two spellings of the same state on one
    /// screen would read as two different things having happened.
    #[test]
    fn a_stranger_that_failed_is_worded_without_an_exit_code_to_quote() {
        let report = StatusReport {
            forms: Vec::new(),
            condition: Condition::Inactive,
            undeclared: vec![Undeclared {
                id: "something-that-fell-over".to_owned(),
                state: State::Failed,
                describes: lemonfiber_core::docker::UNDESCRIBED.to_owned(),
            }],
            services: vec![service("sonarr", State::Absent, None)],
            disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
            unsupported: Vec::new(),
        };
        let text = status(&report).text();

        assert!(text.contains("something-that-fell-over"), "{text}");
        assert!(text.contains("failed"), "{text}");
        assert!(
            !text.contains("exit"),
            "there is no service behind a stranger to ask for a code: {text}"
        );
    }

    /// And an ordinary stack says nothing about it at all, rather than carrying a
    /// heading over an empty list on every run.
    #[test]
    fn a_stack_with_nothing_strange_running_says_nothing_about_strangers() {
        let report = StatusReport {
            forms: Vec::new(),
            condition: Condition::Inactive,
            undeclared: Vec::new(),
            services: vec![service("sonarr", State::Absent, None)],
            disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
            unsupported: Vec::new(),
        };

        assert!(!status(&report).text().contains("does not declare"));
    }

    /// A stack the operator maintains, carrying a declaration lemonfiber cannot reach.
    #[test]
    fn a_service_lemonfiber_cannot_speak_to_is_named_with_why_and_is_not_a_fault() {
        let report = StatusReport {
            forms: Vec::new(),
            condition: Condition::Active,
            services: vec![service("theirs", State::Healthy, None)],
            unsupported: vec![UnsupportedReport {
                what: "theirs".to_owned(),
                because: "it declares an API of the Servarr shape and no port to publish"
                    .to_owned(),
            }],
            undeclared: Vec::new(),
            disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
        };
        let text = status(&report).text();
        assert!(text.contains("needs to know what they are"), "{text}");
        assert!(text.contains("theirs — it declares an API"), "{text}");
        // Still up, and still reported as up: what is unsupported is a feature, not
        // the service.
        assert!(text.contains("everything is up"), "{text}");
    }

    /// And nothing is said at all where there is nothing to say, which is every stack
    /// this build ships.
    #[test]
    fn a_stack_with_nothing_unsupported_says_nothing_about_it() {
        let report = StatusReport {
            forms: Vec::new(),
            condition: Condition::Active,
            services: vec![service("sonarr", State::Healthy, None)],
            unsupported: Vec::new(),
            undeclared: Vec::new(),
            disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
        };
        assert!(!status(&report).text().contains("needs to know"));
    }

    #[test]
    fn a_watch_reports_whether_it_stopped_what_it_guarded() {
        assert!(watch(&a_watch()).text().contains("stopped: media"));
        let stranded = SupervisionReport {
            stopped: false,
            ..a_watch()
        };
        assert!(watch(&stranded).text().contains("could not stop media"));
    }

    #[test]
    fn a_rehearsed_watch_prints_the_invocation_it_would_run_and_the_guard_it_would_keep() {
        let rehearsed = SupervisionReport {
            forms: Vec::new(),
            would: Some(Vigil {
                root: "/srv/library".to_owned(),
                every: 5,
                command: vec!["docker".to_owned(), "compose".to_owned(), "stop".to_owned()],
            }),
            ..a_watch()
        };
        let text = watch(&rehearsed).text();
        assert!(text.contains("docker compose stop"), "{text}");
        assert!(text.contains("/srv/library"), "{text}");
        assert!(text.contains("every 5s"), "{text}");
        assert!(
            text.contains("the whole stack"),
            "naming no form is a guard over all of them: {text}"
        );
        assert!(
            !text.contains("the watch ended"),
            "nothing ended, and saying so would be the rehearsal claiming to be the thing: {text}"
        );
    }

    /// A rehearsed watch over named forms says which ones it would stop.
    ///
    /// The counterpart above names none and gets the sentence for all of them, and both
    /// halves are worth pinning because an operator setting a guard is deciding exactly
    /// this: whether the thing that goes down when the drive does is the part they meant
    /// or the whole stack. A line that ended in nothing would read as a guard that stops
    /// nothing, and one that said "the whole stack" over two named forms would read as a
    /// far bigger promise than the guard makes.
    #[test]
    fn a_rehearsed_watch_over_named_forms_says_which_ones_it_would_stop() {
        let rehearsed = SupervisionReport {
            forms: vec!["library".to_owned(), "search".to_owned()],
            would: Some(Vigil {
                root: "/srv/library".to_owned(),
                every: 5,
                command: vec!["docker".to_owned(), "compose".to_owned(), "stop".to_owned()],
            }),
            ..a_watch()
        };

        let text = watch(&rehearsed).text();

        assert!(text.contains("stopping library, search"), "{text}");
        assert!(
            !text.contains("the whole stack"),
            "a guard over two named forms was described as one over all of them: {text}"
        );
    }

    #[test]
    fn a_watch_read_by_a_script_is_the_envelope_every_other_answer_arrives_in() {
        // Through the outcome rather than through a rendering of its own, so a
        // guard cannot come to describe its ending differently from the rest.
        let json =
            crate::render::machine_readable(&lemonfiber_core::app::Outcome::Watch(a_watch()))
                .text();
        assert!(json.contains(r#""kind":"watch""#), "{json}");
        assert!(json.contains(r#""stopped":true"#), "{json}");
    }
}
