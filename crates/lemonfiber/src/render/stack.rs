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
use lemonfiber_core::stack::closure::{Filtered, Footprint, Plan, Protocol};
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
    let mut lines = affects(plan, Doing::Starting);
    lines.extend(estimate(&plan.footprint));
    lines
}

/// The memory the stack estimates a plan needs, said as the stack's estimate.
///
/// Worded as an estimate every time, because a figure read as a measurement is one an
/// operator believes, and nothing here has measured anything. The services that state
/// no estimate are named, so a sum that leaves them out says so.
fn estimate(footprint: &Footprint) -> Lines {
    let mut lines = Lines::default();
    let silent = footprint.unestimated.join(", ");
    match (footprint.estimated_mib, silent.is_empty()) {
        (0, true) => {}
        (0, false) => lines.put(format!("the stack states no memory estimate for {silent}")),
        (mib, true) => lines.put(format!("the stack estimates about {mib} MiB of memory")),
        (mib, false) => lines.put(format!(
            "the stack estimates about {mib} MiB of memory, leaving out {silent}, \
             which state no estimate"
        )),
    }
    lines
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
    if !report.active_forms.is_empty() {
        lines.put(format!("running for: {}", report.active_forms.join(", ")));
    }
    lines.extend(show(&report.services));
    lines.extend(filtered(&report.filtered));
    lines.extend(strangers(&report.undeclared));
    lines.extend(unsupported(&report.unsupported));
    lines
}

/// What the running forms left out, a service at a time, with what each one wanted.
fn filtered(services: &[Filtered]) -> Lines {
    let mut lines = Lines::default();
    for out in services {
        lines.put(format!(
            "left out: {} — {}",
            plain(&out.name),
            wanting(out.needs)
        ));
    }
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
mod tests;
