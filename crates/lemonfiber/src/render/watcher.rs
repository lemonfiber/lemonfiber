//! What the outside watcher sees, as `lemonfiber watcher` says it.

use lemonfiber_core::model::{WatchPath, WatchState, WatchedTarget, WatcherReport};
use lemonfiber_core::ports::service::{Covers, MonitorKind};

use super::news::moment;
use super::Lines;

/// Which part watches, whether it answers, each monitor beside what it covers, and what
/// nothing watches.
pub(super) fn watcher(report: &WatcherReport) -> Lines {
    let mut lines = Lines::default();
    let read = moment(report.read_at);
    match (&report.watcher, report.reachable) {
        (None, _) => lines.put(format!(
            "Nothing watches this stack from outside it, as of {read}."
        )),
        (Some(part), true) => lines.put(format!("{part} watches this stack, as of {read}.")),
        (Some(part), false) => lines.put(format!(
            "{part} does not answer, so nobody can see what it watches, as of {read}."
        )),
    }
    if let Some(why) = &report.unreachable_because {
        lines.put(format!("  {why}"));
    }
    for monitor in &report.monitors {
        watched(&mut lines, monitor);
    }
    if !report.uncovered.is_empty() {
        lines.spaced("Nothing actively watches:");
        for covers in &report.uncovered {
            lines.put(format!("  {}", covered(covers)));
        }
    }
    lines
}

/// One monitor: its name, what it covers, and what it sees.
fn watched(lines: &mut Lines, monitor: &WatchedTarget) {
    let covers = monitor
        .covers
        .as_ref()
        .map_or_else(|| "not something the stack runs".to_owned(), covered);
    lines.spaced(format!(
        "{} ({}), {covers}: {} since {}",
        monitor.name,
        kind(monitor.kind),
        state(monitor.state),
        moment(monitor.since)
    ));
    if !monitor.active {
        lines.put("  defined, and not checking");
    }
    if let Some(path) = monitor.path {
        lines.put(match path {
            WatchPath::Household => "  over the address the household is handed",
            WatchPath::Internal => "  over the stack's own network, not the household's address",
        });
    }
    if let Some(beat) = &monitor.heartbeat {
        lines.put(format!(
            "  expects a check-in every {} s, {} s grace; {}",
            beat.every_s,
            beat.grace_s,
            beat.last_check_in.map_or_else(
                || "it has never checked in".to_owned(),
                |at| format!("last checked in {}", moment(at))
            )
        ));
    }
    if let Some(unconfigured) = &monitor.unconfigured {
        lines.put(format!(
            "  asked to watch {}, watches {}: {}",
            unconfigured.wanted, unconfigured.watches, unconfigured.why
        ));
    }
}

/// What a monitor covers, in words.
fn covered(covers: &Covers) -> String {
    match covers {
        Covers::Service(id) => format!("the service {id}"),
        Covers::EntryPoint(id) => format!("the household's way in at {id}"),
        Covers::Job(id) => format!("the job {id}"),
    }
}

/// How a monitor checks, in a word.
const fn kind(kind: MonitorKind) -> &'static str {
    match kind {
        MonitorKind::Http => "http",
        MonitorKind::Keyword => "keyword",
        MonitorKind::Port => "port",
        MonitorKind::Ping => "ping",
        MonitorKind::Dns => "dns",
        MonitorKind::Docker => "docker",
        MonitorKind::Heartbeat => "heartbeat",
    }
}

/// What a monitor sees, in words.
const fn state(state: WatchState) -> &'static str {
    match state {
        WatchState::Up => "up",
        WatchState::Down => "down",
        WatchState::Pending => "failing, not yet alarmed",
        WatchState::Paused => "paused",
        WatchState::ExpectedDown => "down, as the stack was stopped on purpose",
        WatchState::NotReporting => "not reporting",
    }
}

#[cfg(test)]
mod tests;
