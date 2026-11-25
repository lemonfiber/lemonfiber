//! The backup or restore running on this machine, as a terminal says it.

use lemonfiber_core::app::Outcome;
use lemonfiber_core::bytes::{a_second, humanize};
use lemonfiber_core::copying::{Copying, State, What};

use super::archive::{copied_name, restoration};
use super::Lines;

/// What a restore put back or would, or the copy running now.
pub(super) fn copied(outcome: &Outcome) -> Lines {
    match outcome {
        Outcome::Restore(report) => restoration(report),
        Outcome::Copying(report) => copying(report),
        _ => Lines::default(),
    }
}

/// The copy running now, how far it has got and how it is moving, or that none is.
pub(crate) fn copying(report: &Copying) -> Lines {
    let mut lines = Lines::default();
    let Some(running) = &report.running else {
        lines.put("No backup or restore is running on this machine.");
        return lines;
    };
    let what = match running.what {
        What::Backup => "A backup",
        What::Restore => "A restore",
    };
    lines.put(format!(
        "{what} of {} has been running since {}.",
        copied_name(&running.scope),
        running.started.said()
    ));
    lines.put(format!(
        "Written {} of {}.",
        humanize(running.written),
        humanize(running.whole)
    ));
    if !running.part.is_empty() {
        lines.put(format!("Writing {} now.", running.part));
    }
    lines.put(match running.state {
        State::Working => "It is writing as fast as it can.".to_owned(),
        State::Paced { limit } => format!(
            "It is slowed on purpose, to at most {} as LEMONFIBER_BACKUP_LIMIT holds.",
            a_second(limit)
        ),
        State::Stalled { wrote } => format!(
            "It is stalled: nothing has been written since {}.",
            wrote.said()
        ),
    });
    if let Some(job) = &running.job {
        lines.put(format!("It runs as job {job}."));
    }
    lines
}

#[cfg(test)]
mod tests;
