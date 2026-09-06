//! What this machine keeps running, said so that installed and running are two things.
//!
//! The whole point of the reading is that a written definition is not a running
//! command, so no line here says one where it means the other. A state the manager
//! would not confirm reads as unconfirmed and never as working, and a command
//! nothing is keeping says outright that it stops when its window closes.

use lemonfiber_core::model::{Changed, HostedCommand, Hosting, HostingReport};

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

#[cfg(test)]
mod tests {
    use super::{hosting, stands, Changed, HostedCommand, Hosting, HostingReport};
    use lemonfiber_core::ports::hosting::Manager;
    use std::path::PathBuf;

    fn a_command(name: &str, standing: Hosting) -> HostedCommand {
        HostedCommand {
            name: name.to_owned(),
            guarantees: "keeps a promise".to_owned(),
            command: format!("lemonfiber {name}"),
            standing,
            definition: None,
            runs: None,
            output: None,
            missing: None,
        }
    }

    fn a_report(manager: Manager, commands: Vec<HostedCommand>) -> HostingReport {
        HostingReport {
            manager,
            commands,
            changed: None,
            instruction: None,
            caveat: None,
        }
    }

    #[test]
    fn a_command_nothing_keeps_running_says_it_stops_with_its_window() {
        let report = a_report(
            Manager::Launchd,
            vec![a_command("watch", Hosting::NotHosted)],
        );
        let text = hosting(&report).text();
        assert!(text.contains("launchd"));
        assert!(text.contains("watch — keeps a promise"));
        assert!(text.contains("stops when the window"));
        assert!(text.contains("would run: lemonfiber watch"));
    }

    #[test]
    fn a_state_the_machine_would_not_confirm_is_never_shown_as_working() {
        let unverified = hosting(&a_report(
            Manager::Systemd,
            vec![a_command("expiring", Hosting::InstalledUnverified)],
        ))
        .text();
        assert!(unverified.contains("would not say whether it is running"));
        assert!(!unverified.contains("this machine is keeping it running"));
    }

    #[test]
    fn every_state_has_words_of_its_own() {
        let every = [
            Hosting::NotHosted,
            Hosting::Hosted,
            Hosting::InstalledUnverified,
            Hosting::Stopped,
            Hosting::Orphaned,
            Hosting::Unsupported,
        ];
        let mut said: Vec<&str> = every.iter().map(|one| stands(*one)).collect();
        let held = said.len();
        said.sort_unstable();
        said.dedup();
        assert_eq!(said.len(), held);
    }

    #[test]
    fn everything_a_definition_holds_is_shown_beneath_it() {
        let report = a_report(
            Manager::Launchd,
            vec![HostedCommand {
                definition: Some(PathBuf::from("/agents/com.lemonfiber.watch.plist")),
                runs: Some("/usr/local/bin/lemonfiber watch tv".to_owned()),
                output: Some(PathBuf::from("/records/watch.log")),
                missing: Some(PathBuf::from("/gone/lemonfiber")),
                ..a_command("watch", Hosting::Orphaned)
            }],
        );
        let text = hosting(&report).text();
        assert!(text.contains("runs: /usr/local/bin/lemonfiber watch tv"));
        assert!(text.contains("written to /records/watch.log"));
        assert!(text.contains("installed as /agents/com.lemonfiber.watch.plist"));
        assert!(text.contains("not there any more: /gone/lemonfiber"));
    }

    #[test]
    fn a_platform_that_cannot_be_configured_is_told_what_to_do_instead() {
        let report = HostingReport {
            instruction: Some("arrange it yourself".to_owned()),
            ..a_report(
                Manager::Unsupported,
                vec![a_command("watch", Hosting::Unsupported)],
            )
        };
        let text = hosting(&report).text();
        assert!(text.contains("none"));
        assert!(text.contains("arrange it yourself"));
    }

    #[test]
    fn a_manager_with_something_worth_knowing_says_it_before_it_is_relied_on() {
        let report = HostingReport {
            caveat: Some("it ends at logout".to_owned()),
            ..a_report(Manager::Systemd, Vec::new())
        };
        assert!(hosting(&report).text().contains("it ends at logout"));
    }

    #[test]
    fn an_install_says_that_it_started_it_and_names_what_it_wrote() {
        let report = HostingReport {
            changed: Some(Changed {
                name: "watch".to_owned(),
                installed: true,
                touched: vec![PathBuf::from("/agents/com.lemonfiber.watch.plist")],
                started: true,
                rehearsed: false,
            }),
            ..a_report(Manager::Launchd, vec![a_command("watch", Hosting::Hosted)])
        };
        let text = hosting(&report).text();
        assert!(text.contains("installed watch"));
        assert!(text.contains("started it — watch is running now"));
        assert!(text.contains("/agents/com.lemonfiber.watch.plist"));
    }

    #[test]
    fn a_removal_names_what_went_and_says_so_where_nothing_had_to() {
        let removed = HostingReport {
            changed: Some(Changed {
                name: "watch".to_owned(),
                installed: false,
                touched: vec![PathBuf::from("/agents/com.lemonfiber.watch.plist")],
                started: false,
                rehearsed: false,
            }),
            ..a_report(Manager::Launchd, Vec::new())
        };
        let text = hosting(&removed).text();
        assert!(text.contains("removed watch"));
        assert!(text.contains("/agents/com.lemonfiber.watch.plist"));

        let nothing = HostingReport {
            changed: Some(Changed {
                name: "watch".to_owned(),
                installed: false,
                touched: Vec::new(),
                started: false,
                rehearsed: false,
            }),
            ..a_report(Manager::Launchd, Vec::new())
        };
        assert!(hosting(&nothing)
            .text()
            .contains("nothing was installed for watch"));
    }

    #[test]
    fn a_rehearsal_says_what_it_would_do_and_never_that_it_did_it() {
        let installing = HostingReport {
            changed: Some(Changed {
                name: "expiring".to_owned(),
                installed: true,
                touched: Vec::new(),
                started: false,
                rehearsed: true,
            }),
            ..a_report(Manager::Systemd, Vec::new())
        };
        let text = hosting(&installing).text();
        assert!(text.contains("would install expiring"));
        assert!(!text.contains("installed expiring"));

        let removing = HostingReport {
            changed: Some(Changed {
                name: "watch".to_owned(),
                installed: false,
                touched: vec![PathBuf::from("/agents/one.plist")],
                started: false,
                rehearsed: true,
            }),
            ..a_report(Manager::Launchd, Vec::new())
        };
        assert!(hosting(&removing).text().contains("would remove watch"));

        let nothing = HostingReport {
            changed: Some(Changed {
                name: "watch".to_owned(),
                installed: false,
                touched: Vec::new(),
                started: false,
                rehearsed: true,
            }),
            ..a_report(Manager::Launchd, Vec::new())
        };
        assert!(hosting(&nothing)
            .text()
            .contains("nothing is installed for watch"));
    }

    #[test]
    fn an_install_that_did_not_start_it_does_not_say_it_did() {
        let report = HostingReport {
            changed: Some(Changed {
                name: "watch".to_owned(),
                installed: true,
                touched: vec![PathBuf::from("/agents/one.plist")],
                started: false,
                rehearsed: false,
            }),
            ..a_report(Manager::Launchd, Vec::new())
        };
        let text = hosting(&report).text();
        assert!(text.contains("installed watch"));
        assert!(!text.contains("running now"));
    }
}
