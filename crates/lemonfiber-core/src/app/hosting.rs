//! Handing a long-running command to this machine, and taking it back.
//!
//! Two of this product's guarantees are made by a command that has to keep
//! running, and both of them end when the terminal that started them closes.
//! This is what makes them survive it: the operating system's own service
//! manager is asked to run the same command the operator would have typed.
//!
//! Nothing here reports success from having written a file. What a reading says
//! is what the manager answered, and where the manager would not answer, that is
//! its own state and says so — because the failure this exists to remove is an
//! operator believing a guarantee is in force while nothing is keeping it.
//!
//! Installing is asked for and never arrived at. No other command reaches this,
//! and running one of the two long commands does not offer it: an operator who
//! asked to guard a volume this afternoon has not asked for something on their
//! machine that starts at every login.

use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};
use crate::model::{Changed, HostedCommand, Hosting, HostingReport};
use crate::ports::hosting::{Held, Hosted, Manager, Standing};

use super::command::{Hostable, Keeping, HOSTABLE};
use super::Ctx;

/// Raised when this machine will not say where it keeps its own files.
pub const NOWHERE_TO_WRITE: Code = Code::new("HOST-4");

/// Raised when this run cannot say where its own program is.
pub const NO_PROGRAM: Code = Code::new("HOST-5");

/// Raised when the guard is to be hosted against nothing.
pub const NOTHING_NAMED_TO_GUARD: Code = Code::new("HOST-6");

/// What a systemd user session does not do, said before it is relied on.
const UNTIL_LOGOUT: &str = "A user service runs while you are logged in. Surviving a logout \
                            needs lingering turned on for your account, which is a setting on \
                            the account rather than on this service — lemonfiber does not turn \
                            it on for you.";

/// What to do where lemonfiber configures nothing on this platform.
const INSTEAD: &str = "lemonfiber does not configure this platform's way of starting things. \
                       Arrange it with whatever this system uses to run a program at login, \
                       naming the command exactly as it reads above.";

/// Say what is hosted, or change it and then say.
///
/// The reading is taken after the change rather than assembled from it, so what
/// is reported is what the manager holds now and never what a write claimed.
///
/// # Errors
///
/// Returns a [`Problem`] where an install or a removal could not be carried out:
/// a platform with no manager, a manager that refused, a machine that will not
/// say where it keeps its files, or a guard named against no forms.
pub(super) async fn hosting(ctx: &Ctx, asked: Keeping) -> Result<HostingReport, Box<Problem>> {
    let changed = match asked {
        Keeping::Read => None,
        Keeping::Install { what, forms } => Some(install(ctx, what, &forms).await?),
        Keeping::Remove { what } => Some(remove(ctx, what).await?),
    };
    Ok(reading(ctx, changed).await)
}

/// What this machine keeps running, as it stands now.
async fn reading(ctx: &Ctx, changed: Option<Changed>) -> HostingReport {
    let manager = ctx.hosting.manager();
    let mut commands = Vec::with_capacity(HOSTABLE.len());
    for what in HOSTABLE {
        commands.push(described(ctx, what).await);
    }
    HostingReport {
        manager,
        commands,
        changed,
        instruction: (!manager.configurable()).then(|| INSTEAD.to_owned()),
        caveat: matches!(manager, Manager::Systemd).then(|| UNTIL_LOGOUT.to_owned()),
    }
}

/// One command, and what stands between it and the machine.
async fn described(ctx: &Ctx, what: Hostable) -> HostedCommand {
    // The one thing a manager refuses to answer about is a platform it is not on,
    // which is a state of the machine rather than of this command.
    let (standing, held) = match ctx.hosting.standing(what.name()).await {
        Err(_) => (Hosting::Unsupported, Held::absent()),
        Ok(held) => (settled(&held), held),
    };
    HostedCommand {
        name: what.name().to_owned(),
        guarantees: what.guarantees().to_owned(),
        command: typed(what, &[]),
        standing,
        definition: held.definition,
        runs: held.runs,
        output: held.output,
        missing: held
            .program
            .filter(|program| !program.present)
            .map(|program| program.at),
    }
}

/// Whether this machine is running one of them, rather than whether one is installed.
///
/// The two are different facts everywhere in this module, and they are different
/// here for the reason that matters most: what reads this is a promise made to a
/// household, and a definition sitting on disk that the manager is not running
/// keeps none of it. A manager that will not say is not one to promise on either,
/// so anything short of a confirmed run reads as nothing running it.
pub(super) async fn keeping(ctx: &Ctx, what: Hostable) -> bool {
    ctx.hosting
        .standing(what.name())
        .await
        .is_ok_and(|held| held.standing == Standing::Running)
}

/// What the manager's answer amounts to.
fn settled(held: &Held) -> Hosting {
    match held.standing {
        Standing::Absent => Hosting::NotHosted,
        _ if held.orphaned() => Hosting::Orphaned,
        Standing::Running => Hosting::Hosted,
        Standing::Stopped => Hosting::Stopped,
        Standing::Unsaid => Hosting::InstalledUnverified,
    }
}

/// The command as it would be typed at a terminal.
fn typed(what: Hostable, forms: &[String]) -> String {
    format!("lemonfiber {}", what.arguments(forms).join(" "))
}

/// Hand one to the machine.
async fn install(ctx: &Ctx, what: Hostable, forms: &[String]) -> Result<Changed, Box<Problem>> {
    let wanted = wanted(ctx, what, forms)?;
    if ctx.dry_run {
        return Ok(Changed {
            name: what.name().to_owned(),
            installed: true,
            touched: Vec::new(),
            started: false,
            rehearsed: true,
        });
    }
    let placed = ctx
        .hosting
        .place(&wanted)
        .await
        .map_err(|failure| Box::new(failure.problem()))?;
    Ok(Changed {
        name: what.name().to_owned(),
        installed: true,
        touched: vec![placed.definition],
        started: placed.started,
        rehearsed: false,
    })
}

/// Take one back off the machine.
async fn remove(ctx: &Ctx, what: Hostable) -> Result<Changed, Box<Problem>> {
    if ctx.dry_run {
        let held = ctx
            .hosting
            .standing(what.name())
            .await
            .unwrap_or_else(|_| Held::absent());
        return Ok(Changed {
            name: what.name().to_owned(),
            installed: false,
            touched: held.definition.into_iter().collect(),
            started: false,
            rehearsed: true,
        });
    }
    let taken = ctx
        .hosting
        .withdraw(what.name())
        .await
        .map_err(|failure| Box::new(failure.problem()))?;
    Ok(Changed {
        name: what.name().to_owned(),
        installed: false,
        touched: taken,
        started: false,
        rehearsed: false,
    })
}

/// What is to be installed, or why it cannot be described.
fn wanted(ctx: &Ctx, what: Hostable, forms: &[String]) -> Result<Hosted, Box<Problem>> {
    if what.takes_forms() && forms.is_empty() {
        return Err(Box::new(nothing_to_guard()));
    }
    let Some(program) = ctx.settings.program.clone() else {
        return Err(Box::new(no_program()));
    };
    let Some(hosted) = ctx.settings.hosted.clone() else {
        return Err(Box::new(nowhere_to_write()));
    };
    Ok(Hosted {
        name: what.name().to_owned(),
        program,
        arguments: what.arguments(forms),
        output: hosted.join(format!("{}.log", what.name())),
        about: what.guarantees().to_owned(),
    })
}

/// The problem for a guard to be hosted against nothing.
fn nothing_to_guard() -> Problem {
    Problem::new(
        NOTHING_NAMED_TO_GUARD,
        Severity::Error,
        "The guard was not told what to guard",
        "A guard stops the forms it was started against, so one started against none would \
         watch the data location and then have nothing to stop.",
        Remedy::new("Name the forms to guard, as you would when running the guard yourself"),
    )
    .in_state(State::Guided)
}

/// The problem for a run that cannot say where its own program is.
fn no_program() -> Problem {
    Problem::new(
        NO_PROGRAM,
        Severity::Error,
        "This machine would not say where lemonfiber itself is",
        "A service names the program it runs, and one naming a guessed path would fail at \
         every login with nothing to say why.",
        Remedy::new("Run this again from an installed copy of lemonfiber rather than a piped one"),
    )
    .in_state(State::Guided)
}

/// The problem for a run with nowhere to put a hosted command's words.
fn nowhere_to_write() -> Problem {
    Problem::new(
        NOWHERE_TO_WRITE,
        Severity::Error,
        "This machine would not say where lemonfiber keeps its files",
        "A hosted command has no terminal to speak in, so without somewhere to write what it \
         says, nothing it did would be readable afterwards.",
        Remedy::new("Set a home directory for this account, then install it again"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests {
    use super::{
        hosting, keeping, settled, typed, Ctx, Held, Hostable, Hosting, Keeping, Standing,
    };
    use crate::config::Settings;
    use crate::model::HostingReport;
    use crate::ports::hosting::{Manager, Program};
    use crate::ports::Host;
    use lemonfiber_fixtures::hosting::Hosting as Fake;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    /// Settings for a machine that knows where it is and what it is.
    fn knowing() -> Settings {
        Settings {
            program: Some(PathBuf::from("/usr/local/bin/lemonfiber")),
            hosted: Some(PathBuf::from("/records")),
            ..Settings::default()
        }
    }

    /// A machine with these settings and this manager.
    fn machine(settings: Settings, manager: Arc<Fake>) -> Ctx {
        crate::test_support::a_context()
            .settings(settings)
            .build()
            .hosting_with(manager as Arc<dyn Host>)
    }

    /// A machine that knows where it is, with this manager.
    fn a_machine(manager: Arc<Fake>) -> Ctx {
        machine(knowing(), manager)
    }

    /// The report, or the empty one — which no assertion below is satisfied by, so
    /// a run that failed where it should not fails the test rather than skipping it.
    async fn read(ctx: &Ctx, asked: Keeping) -> HostingReport {
        hosting(ctx, asked).await.unwrap_or_default()
    }

    /// The state one command reads as.
    fn standing(report: &HostingReport, name: &str) -> Option<Hosting> {
        report
            .commands
            .iter()
            .find(|command| command.name == name)
            .map(|command| command.standing)
    }

    #[tokio::test]
    async fn every_long_running_command_is_on_the_reading_whether_hosted_or_not() {
        let report = read(&a_machine(Fake::with(Manager::Launchd)), Keeping::Read).await;
        assert_eq!(report.commands.len(), 2);
        assert_eq!(standing(&report, "watch"), Some(Hosting::NotHosted));
        assert_eq!(standing(&report, "expiring"), Some(Hosting::NotHosted));
        assert_eq!(report.manager, Manager::Launchd);
        assert_eq!(report.instruction, None);
        assert_eq!(report.caveat, None);
        assert_eq!(report.changed, None);
        assert!(report.commands.iter().all(|command| {
            command.command.starts_with("lemonfiber ") && !command.guarantees.is_empty()
        }));
    }

    #[tokio::test]
    async fn a_platform_with_no_manager_is_told_what_to_do_instead() {
        let report = read(&a_machine(Fake::unsupported()), Keeping::Read).await;
        assert_eq!(report.manager, Manager::Unsupported);
        assert_eq!(standing(&report, "watch"), Some(Hosting::Unsupported));
        assert_eq!(standing(&report, "expiring"), Some(Hosting::Unsupported));
        assert!(report
            .instruction
            .is_some_and(|said| said.contains("at login")));
    }

    #[tokio::test]
    async fn a_user_service_says_what_it_does_not_survive_before_it_is_relied_on() {
        let report = read(&a_machine(Fake::with(Manager::Systemd)), Keeping::Read).await;
        assert!(report.caveat.is_some_and(|said| said.contains("lingering")));
    }

    #[tokio::test]
    async fn installing_the_guard_hands_the_machine_the_command_as_it_would_be_typed() {
        let manager = Fake::with(Manager::Launchd);
        let report = read(
            &a_machine(Arc::clone(&manager)),
            Keeping::Install {
                what: Hostable::Watch,
                forms: vec!["tv".to_owned()],
            },
        )
        .await;

        let placed = manager.placed();
        assert_eq!(placed.len(), 1);
        assert!(placed.first().is_some_and(|one| {
            one.name == "watch"
                && one.arguments == vec!["watch".to_owned(), "tv".to_owned()]
                && one.program == Path::new("/usr/local/bin/lemonfiber")
                && one.output == Path::new("/records/watch.log")
        }));
        assert_eq!(standing(&report, "watch"), Some(Hosting::Hosted));
        assert!(report
            .changed
            .is_some_and(|changed| changed.installed && changed.started && !changed.rehearsed));
    }

    #[tokio::test]
    async fn installing_the_clock_needs_no_forms_and_takes_none() {
        let manager = Fake::with(Manager::Systemd);
        let report = read(
            &a_machine(Arc::clone(&manager)),
            Keeping::Install {
                what: Hostable::Expiring,
                forms: Vec::new(),
            },
        )
        .await;
        assert!(manager.placed().first().is_some_and(
            |one| one.arguments == vec!["household".to_owned(), "expiring".to_owned()]
        ));
        assert_eq!(standing(&report, "expiring"), Some(Hosting::Hosted));
    }

    #[tokio::test]
    async fn a_guard_named_against_nothing_is_refused_by_name() {
        let refused = hosting(
            &a_machine(Fake::with(Manager::Launchd)),
            Keeping::Install {
                what: Hostable::Watch,
                forms: Vec::new(),
            },
        )
        .await;
        assert!(refused.is_err_and(|problem| problem.code.as_str() == "HOST-6"));
    }

    #[tokio::test]
    async fn a_machine_that_will_not_say_where_it_keeps_things_installs_nothing() {
        let without_program = machine(
            Settings {
                program: None,
                ..knowing()
            },
            Fake::with(Manager::Launchd),
        );
        assert!(hosting(
            &without_program,
            Keeping::Install {
                what: Hostable::Expiring,
                forms: Vec::new()
            }
        )
        .await
        .is_err_and(|problem| problem.code.as_str() == "HOST-5"));

        let without_records = machine(
            Settings {
                hosted: None,
                ..knowing()
            },
            Fake::with(Manager::Launchd),
        );
        assert!(hosting(
            &without_records,
            Keeping::Install {
                what: Hostable::Expiring,
                forms: Vec::new()
            }
        )
        .await
        .is_err_and(|problem| problem.code.as_str() == "HOST-4"));
    }

    #[tokio::test]
    async fn a_manager_that_refused_is_reported_as_the_refusal_it_is() {
        let refused = hosting(
            &a_machine(Fake::refusing(Manager::Launchd, "Load failed: 5")),
            Keeping::Install {
                what: Hostable::Expiring,
                forms: Vec::new(),
            },
        )
        .await;
        assert!(refused.is_err_and(|problem| problem.code.as_str() == "HOST-3"));

        let nowhere = hosting(
            &a_machine(Fake::unsupported()),
            Keeping::Install {
                what: Hostable::Expiring,
                forms: Vec::new(),
            },
        )
        .await;
        assert!(nowhere.is_err_and(|problem| problem.code.as_str() == "HOST-1"));
    }

    #[tokio::test]
    async fn taking_one_back_names_what_went_and_leaves_it_reading_as_unhosted() {
        let manager = Fake::with(Manager::Systemd);
        let ctx = a_machine(Arc::clone(&manager));
        assert!(hosting(
            &ctx,
            Keeping::Install {
                what: Hostable::Expiring,
                forms: Vec::new()
            }
        )
        .await
        .is_ok());

        let report = read(
            &ctx,
            Keeping::Remove {
                what: Hostable::Expiring,
            },
        )
        .await;
        assert_eq!(manager.withdrawn(), vec!["expiring".to_owned()]);
        assert_eq!(standing(&report, "expiring"), Some(Hosting::NotHosted));
        assert!(report.changed.is_some_and(|changed| {
            !changed.installed && changed.touched.len() == 1 && !changed.started
        }));
    }

    #[tokio::test]
    async fn taking_back_what_was_never_installed_says_so_and_does_not_fail() {
        let report = read(
            &a_machine(Fake::with(Manager::Systemd)),
            Keeping::Remove {
                what: Hostable::Watch,
            },
        )
        .await;
        assert!(report
            .changed
            .is_some_and(|changed| !changed.installed && changed.touched.is_empty()));
    }

    #[tokio::test]
    async fn a_removal_a_manager_refused_is_reported_rather_than_claimed() {
        let refused = hosting(
            &a_machine(Fake::refusing(Manager::Systemd, "it is still running")),
            Keeping::Remove {
                what: Hostable::Watch,
            },
        )
        .await;
        assert!(refused.is_err_and(|problem| problem.code.as_str() == "HOST-3"));
    }

    #[tokio::test]
    async fn a_rehearsal_changes_nothing_and_still_names_what_it_would_have_changed() {
        let manager = Fake::holding(
            Manager::Launchd,
            "watch",
            Fake::installed("watch", Standing::Running),
        );
        let ctx = a_machine(Arc::clone(&manager)).rehearsing();

        let installed = read(
            &ctx,
            Keeping::Install {
                what: Hostable::Expiring,
                forms: Vec::new(),
            },
        )
        .await;
        assert!(manager.placed().is_empty());
        assert!(installed
            .changed
            .is_some_and(|changed| changed.rehearsed && !changed.started));

        let removed = read(
            &ctx,
            Keeping::Remove {
                what: Hostable::Watch,
            },
        )
        .await;
        assert!(manager.withdrawn().is_empty());
        assert_eq!(standing(&removed, "watch"), Some(Hosting::Hosted));
        assert!(removed
            .changed
            .is_some_and(|changed| changed.rehearsed && changed.touched.len() == 1));
    }

    #[tokio::test]
    async fn a_rehearsed_removal_on_a_platform_with_no_manager_names_nothing() {
        let ctx = a_machine(Fake::unsupported()).rehearsing();
        let report = read(
            &ctx,
            Keeping::Remove {
                what: Hostable::Watch,
            },
        )
        .await;
        assert!(report
            .changed
            .is_some_and(|changed| changed.rehearsed && changed.touched.is_empty()));
    }

    #[test]
    fn every_answer_a_manager_can_give_becomes_one_of_the_six_states() {
        let installed = |standing| Held {
            standing,
            definition: Some(PathBuf::from("/services/one")),
            program: Some(Program {
                at: PathBuf::from("/usr/local/bin/lemonfiber"),
                present: true,
            }),
            runs: Some("lemonfiber watch".to_owned()),
            output: None,
        };
        assert_eq!(settled(&Held::absent()), Hosting::NotHosted);
        assert_eq!(settled(&installed(Standing::Running)), Hosting::Hosted);
        assert_eq!(settled(&installed(Standing::Stopped)), Hosting::Stopped);
        assert_eq!(
            settled(&installed(Standing::Unsaid)),
            Hosting::InstalledUnverified
        );
        let gone = Held {
            program: Some(Program {
                at: PathBuf::from("/gone/lemonfiber"),
                present: false,
            }),
            ..installed(Standing::Running)
        };
        assert_eq!(settled(&gone), Hosting::Orphaned);
    }

    #[tokio::test]
    async fn a_definition_naming_a_program_that_has_gone_reads_as_orphaned() {
        let manager = Fake::holding(Manager::Launchd, "watch", Fake::orphaned("watch"));
        let report = read(&a_machine(manager), Keeping::Read).await;
        assert_eq!(standing(&report, "watch"), Some(Hosting::Orphaned));
        assert!(report
            .commands
            .iter()
            .any(|command| command.missing == Some(PathBuf::from("/gone/lemonfiber"))));
    }

    #[tokio::test]
    async fn what_a_definition_holds_is_carried_onto_the_reading() {
        let manager = Fake::holding(
            Manager::Systemd,
            "expiring",
            Fake::installed("expiring", Standing::Unsaid),
        );
        let report = read(&a_machine(manager), Keeping::Read).await;
        assert_eq!(
            standing(&report, "expiring"),
            Some(Hosting::InstalledUnverified)
        );
        assert!(report.commands.iter().any(|command| {
            command.name == "expiring"
                && command.definition == Some(PathBuf::from("/services/lemonfiber-expiring"))
                && command.runs.as_deref() == Some("/usr/local/bin/lemonfiber expiring")
                && command.output == Some(PathBuf::from("/records/expiring.log"))
                && command.missing.is_none()
        }));
    }

    /// The dispatcher reaches this, and from the copy of the app layer compiled here.
    ///
    /// Driven from `tests/` as well, because the layer is compiled twice and a path
    /// exercised in one is counted as never run in the other. Both are the same arm
    /// and neither stands in for the other.
    #[tokio::test]
    async fn the_dispatcher_reaches_the_reading() {
        let asked = crate::app::dispatch(
            crate::app::Command::Hosting(Keeping::Read),
            &a_machine(Fake::with(Manager::Launchd)),
        )
        .await;
        assert!(matches!(
            asked,
            Ok(crate::app::Outcome::Hosting(report)) if report.commands.len() == 2
        ));
    }

    /// Only a manager that confirms a run counts as keeping one going.
    ///
    /// What reads this is a promise made to a household, so the three answers short
    /// of a confirmed run — installed and stopped, installed and unsaid, and no
    /// manager at all — each come back the same way, and it is the honest way.
    #[tokio::test]
    async fn a_machine_keeps_one_going_only_where_the_manager_says_it_is_running() {
        let holding = |standing| {
            Fake::holding(
                Manager::Systemd,
                "expiring",
                Fake::installed("expiring", standing),
            )
        };
        assert!(keeping(&a_machine(holding(Standing::Running)), Hostable::Expiring).await);
        assert!(!keeping(&a_machine(holding(Standing::Stopped)), Hostable::Expiring).await);
        assert!(!keeping(&a_machine(holding(Standing::Unsaid)), Hostable::Expiring).await);
        assert!(!keeping(&a_machine(Fake::unsupported()), Hostable::Expiring).await);
    }

    #[test]
    fn what_is_installed_is_the_command_as_it_would_be_typed() {
        assert_eq!(
            typed(Hostable::Watch, &["tv".to_owned()]),
            "lemonfiber watch tv"
        );
        assert_eq!(
            typed(Hostable::Expiring, &[]),
            "lemonfiber household expiring"
        );
    }
}
