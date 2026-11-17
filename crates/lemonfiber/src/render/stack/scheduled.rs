//! What the stack's run-and-exit services last did.

use lemonfiber_core::model::ScheduledService;
use lemonfiber_core::plural::s;
use lemonfiber_core::text::plain;

use super::super::Lines;

/// Each run-and-exit service, with how its last run ended and, where it failed, its
/// last lines.
pub(super) fn scheduled(services: &[ScheduledService]) -> Lines {
    let mut lines = Lines::default();
    if services.is_empty() {
        return lines;
    }
    lines.spaced("run on a schedule:");
    for one in services {
        let state = match (&one.last, one.running) {
            (_, true) => "running".to_owned(),
            (None, false) => "not run yet".to_owned(),
            (Some(last), false) => match last.exit {
                Some(0) => format!("ran {}", last.ended.said()),
                Some(code) => format!("failed ({code}) {}", last.ended.said()),
                None => format!("failed {}", last.ended.said()),
            },
        };
        let unfired = if one.fired {
            ""
        } else {
            ", and nothing on this machine runs it"
        };
        lines.put(format!("  {:<14} {state}{unfired}", plain(&one.id)));
        if !one.healthy() {
            for line in one.last.iter().flat_map(|last| &last.log) {
                lines.put(format!("    {}", plain(line)));
            }
        }
        if one.skipped > 0 {
            lines.put(format!(
                "    {} time{} skipped while a run was going",
                one.skipped,
                s(usize::try_from(one.skipped).unwrap_or(usize::MAX))
            ));
        }
    }
    lines
}

#[cfg(test)]
mod tests;
