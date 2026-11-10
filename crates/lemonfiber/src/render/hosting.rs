//! What this machine keeps running, said so that installed and running are two things,
//! and how the stack runs without lemonfiber at all.
//!
//! The whole point of the reading is that a written definition is not a running
//! command, so no line here says one where it means the other. A state the manager
//! would not confirm reads as unconfirmed and never as working, and a command
//! nothing is keeping says outright that it stops when its window closes.

use lemonfiber_core::model::{
    Changed, HostedCommand, Hosting, HostingReport, StackReport, StackSource,
};

use super::Lines;

/// What this machine keeps running, and what this run did about it.
pub(super) fn hosting(report: &HostingReport) -> Lines {
    let mut lines = Lines::default();

    if let Some(changed) = &report.changed {
        said(&mut lines, changed);
        lines.put(String::new());
    }

    lines.put(format!(
        "this machine's way of keeping things running: {}",
        report.manager.named()
    ));
    for command in &report.commands {
        lines.put(String::new());
        described(&mut lines, command);
    }
    if let Some(caveat) = &report.caveat {
        lines.spaced(caveat.clone());
    }
    if let Some(instruction) = &report.instruction {
        lines.spaced(instruction.clone());
    }
    lines
}

/// What this run did, before anything is said about what now stands.
fn said(lines: &mut Lines, changed: &Changed) {
    let name = &changed.name;
    match (changed.installed, changed.rehearsed) {
        (true, true) => lines.put(format!(
            "would install {name}, and nothing has been written"
        )),
        (true, false) => {
            lines.put(format!("installed {name}"));
            if changed.started {
                lines.put(format!("started it — {name} is running now"));
            }
        }
        (false, true) if changed.touched.is_empty() => {
            lines.put(format!(
                "nothing is installed for {name}, so nothing would go"
            ));
        }
        (false, true) => lines.put(format!("would remove {name}, and nothing has been removed")),
        (false, false) if changed.touched.is_empty() => {
            lines.put(format!(
                "nothing was installed for {name}, so nothing was removed"
            ));
        }
        (false, false) => lines.put(format!("removed {name}")),
    }
    for path in &changed.touched {
        lines.put(format!("  {}", path.display()));
    }
}

/// One command, and what stands between it and the machine.
fn described(lines: &mut Lines, command: &HostedCommand) {
    lines.put(format!("{} — {}", command.name, command.guarantees));
    lines.put(format!("  {}", stands(command.standing)));
    if let Some(runs) = &command.runs {
        lines.put(format!("  runs: {runs}"));
    } else {
        lines.put(format!("  would run: {}", command.command));
    }
    if let Some(missing) = &command.missing {
        lines.put(format!(
            "  the program it names is not there any more: {}",
            missing.display()
        ));
    }
    if let Some(output) = &command.output {
        lines.put(format!("  what it says is written to {}", output.display()));
    }
    if let Some(definition) = &command.definition {
        lines.put(format!("  installed as {}", definition.display()));
    }
}

/// What one state means, in the words somebody reading it needs.
const fn stands(standing: Hosting) -> &'static str {
    match standing {
        Hosting::NotHosted => {
            "nothing is keeping this running — it stops when the window it was started in closes"
        }
        Hosting::Hosted => "this machine is keeping it running",
        Hosting::InstalledUnverified => {
            "installed, and this machine would not say whether it is running — so do not \
             count on it until it does"
        }
        Hosting::Stopped => "installed, and not running",
        Hosting::Orphaned => "installed against a program that has since moved, so it runs nothing",
        Hosting::Unsupported => {
            "nothing here can keep this running — it stops when the window it was started in \
             closes"
        }
    }
}

/// How the stack runs without lemonfiber: whose stack it is, where it is written, the
/// words that start it, and what those words alone leave out.
pub(super) fn underneath(report: &StackReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(match &report.source {
        StackSource::Bundled => "The stack this build ships.".to_owned(),
        StackSource::Own(path) => format!("Your own stack, from {path}."),
    });
    lines.put(format!("  Project   {}", report.project));
    lines.put(format!("  Settings  {}", report.env));
    lines.spaced("Without lemonfiber running, this starts it:");
    lines.put(format!("  {}", report.plain_up.join(" ")));
    lines.spaced(format!(
        "Every action runs without a question as {}.",
        report.non_interactive.form
    ));
    if !report.exceptions.is_empty() {
        lines.spaced("Those words alone leave out:");
        for exception in &report.exceptions {
            lines.put(format!("  {}   {}", exception.part, exception.because));
        }
    }
    lines
}

#[cfg(test)]
mod tests;
