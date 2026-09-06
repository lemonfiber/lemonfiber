//! Handing a long-running command to this machine, driven through the dispatcher.
//!
//! From here rather than only from a `#[cfg(test)]` module for the reason the forms
//! listing and the expiry are: the app layer is compiled twice, and a path exercised
//! only in-crate has its coverage counted from the copy that never ran.
//!
//! **What is asserted is what the manager was handed and what came back**, because
//! those are the two halves this feature exists to keep apart. A definition written
//! is not a command running, and a report that read the first and printed the second
//! would be the failure the whole thing is for.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::stack::project;
use lemonfiber_core::adapters::{Daemon, Disk, Local, System};
use lemonfiber_core::app::{dispatch, Command, Ctx, Hostable, Keeping, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::{Hosting, HostingReport};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::hosting::{Manager, Standing};
use lemonfiber_core::ports::Host;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::hosting::Hosting as Fake;

/// A machine that knows where it is, holding the given manager.
fn ctx(manager: Arc<Fake>) -> Ctx {
    Ctx::new(
        Arc::new(Local),
        Arc::new(Daemon::local()),
        Arc::new(System),
        Arc::new(Disk),
        Source::External(project()),
        Settings {
            program: Some(PathBuf::from("/usr/local/bin/lemonfiber")),
            hosted: Some(PathBuf::from("/records")),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .hosting_with(manager as Arc<dyn Host>)
}

/// What one run came to, or the empty report — which satisfies no assertion below.
async fn reading(ctx: &Ctx, asked: Keeping) -> HostingReport {
    match dispatch(Command::Hosting(asked), ctx).await {
        Ok(Outcome::Hosting(report)) => report,
        _ => HostingReport::default(),
    }
}

/// What one command reads as.
fn standing(report: &HostingReport, name: &str) -> Option<Hosting> {
    report
        .commands
        .iter()
        .find(|command| command.name == name)
        .map(|command| command.standing)
}

/// Both of the long commands are on the reading, and neither is hosted until asked.
///
/// The count is asserted before the states, because `all` over an empty list is true
/// and a reading that found nothing would otherwise pass every claim below it.
#[tokio::test]
async fn both_long_running_commands_are_on_the_reading_and_neither_starts_hosted() {
    let report = reading(&ctx(Fake::with(Manager::Launchd)), Keeping::Read).await;

    assert_eq!(report.commands.len(), 2);
    assert_eq!(standing(&report, "watch"), Some(Hosting::NotHosted));
    assert_eq!(standing(&report, "expiring"), Some(Hosting::NotHosted));
    assert!(report
        .commands
        .iter()
        .all(|command| !command.standing.keeping()));
}

/// Installing hands the manager the command as it would have been typed, and the
/// reading afterwards is what the manager says rather than what the install claimed.
#[tokio::test]
async fn installing_the_guard_hands_over_the_command_and_then_reads_it_back() {
    let manager = Fake::with(Manager::Launchd);
    let report = reading(
        &ctx(Arc::clone(&manager)),
        Keeping::Install {
            what: Hostable::Watch,
            forms: vec!["tv".to_owned(), "films".to_owned()],
        },
    )
    .await;

    let placed = manager.placed();
    assert_eq!(placed.len(), 1, "one service, and one only");
    assert!(placed.first().is_some_and(|one| {
        one.arguments == vec!["watch".to_owned(), "tv".to_owned(), "films".to_owned()]
            && one.program == Path::new("/usr/local/bin/lemonfiber")
            && one.output == Path::new("/records/watch.log")
    }));
    assert_eq!(standing(&report, "watch"), Some(Hosting::Hosted));
    assert!(report
        .changed
        .is_some_and(|changed| changed.installed && changed.started && changed.touched.len() == 1));
}

/// The clock is the other one, and one mechanism covers both.
#[tokio::test]
async fn installing_the_clock_goes_through_the_same_mechanism() {
    let manager = Fake::with(Manager::Systemd);
    let report = reading(
        &ctx(Arc::clone(&manager)),
        Keeping::Install {
            what: Hostable::Expiring,
            forms: Vec::new(),
        },
    )
    .await;

    assert!(manager
        .placed()
        .first()
        .is_some_and(|one| one.arguments == vec!["household".to_owned(), "expiring".to_owned()]));
    assert_eq!(standing(&report, "expiring"), Some(Hosting::Hosted));
    assert!(
        report.caveat.is_some(),
        "a user service that ends at logout says so before it is relied on"
    );
}

/// A manager that will not say is never reported as keeping anything.
///
/// The state that matters most: installed and running are two facts, and the one
/// where somebody believes a guarantee is in force is the one nothing may round up.
#[tokio::test]
async fn a_manager_that_would_not_say_is_not_reported_as_running() {
    let manager = Fake::holding(
        Manager::Launchd,
        "expiring",
        Fake::installed("expiring", Standing::Unsaid),
    );
    let report = reading(&ctx(manager), Keeping::Read).await;

    assert_eq!(
        standing(&report, "expiring"),
        Some(Hosting::InstalledUnverified)
    );
    assert!(standing(&report, "expiring")
        .is_some_and(|standing| standing.installed() && !standing.keeping()));
}

/// Taking one back names what went, and the reading afterwards says nothing is left.
#[tokio::test]
async fn taking_one_back_names_what_went_and_leaves_nothing_behind() {
    let manager = Fake::with(Manager::Launchd);
    let ctx = ctx(Arc::clone(&manager));
    assert!(dispatch(
        Command::Hosting(Keeping::Install {
            what: Hostable::Expiring,
            forms: Vec::new(),
        }),
        &ctx,
    )
    .await
    .is_ok());

    let report = reading(
        &ctx,
        Keeping::Remove {
            what: Hostable::Expiring,
        },
    )
    .await;

    assert_eq!(manager.withdrawn(), vec!["expiring".to_owned()]);
    assert_eq!(standing(&report, "expiring"), Some(Hosting::NotHosted));
    assert!(report
        .changed
        .is_some_and(|changed| !changed.installed && changed.touched.len() == 1));
}

/// A platform with no manager is told what to do instead, and nothing is claimed.
#[tokio::test]
async fn a_platform_with_no_manager_is_instructed_rather_than_told_it_is_installed() {
    let read = reading(&ctx(Fake::unsupported()), Keeping::Read).await;
    assert_eq!(read.manager, Manager::Unsupported);
    assert!(read.instruction.is_some());
    assert!(read
        .commands
        .iter()
        .all(|command| command.standing == Hosting::Unsupported && command.definition.is_none()));

    let refused = dispatch(
        Command::Hosting(Keeping::Install {
            what: Hostable::Expiring,
            forms: Vec::new(),
        }),
        &ctx(Fake::unsupported()),
    )
    .await;
    assert!(refused.is_err_and(|problem| problem.code.as_str() == "HOST-1"));
}

/// The manager a context nobody told carries, driven rather than described.
///
/// Told where it is and what it is, because a run that could not say either is
/// refused for that before a manager is ever reached — and the refusal under test
/// here is the manager's own.
///
/// `Ctx::new` defaults to the one that configures nothing, and every reading in
/// this workspace that has not been handed a manager goes through it — so what it
/// does is worth driving rather than assuming. It is also the only way to reach
/// that implementation from the crate's ordinary compilation: the tests beside the
/// code drive a fake, and a path exercised only there is counted as never run here.
#[tokio::test]
async fn a_context_nobody_told_hosts_nothing_and_refuses_to_pretend() {
    let bare = || {
        Ctx::new(
            Arc::new(Local),
            Arc::new(Daemon::local()),
            Arc::new(System),
            Arc::new(Disk),
            Source::External(project()),
            Settings {
                program: Some(PathBuf::from("/usr/local/bin/lemonfiber")),
                hosted: Some(PathBuf::from("/records")),
                ..Settings::default()
            },
            Environment::MacOs,
        )
    };

    let read = reading(&bare(), Keeping::Read).await;
    assert_eq!(read.manager, Manager::Unsupported);
    assert!(read
        .commands
        .iter()
        .all(|command| command.standing == Hosting::Unsupported));

    for asked in [
        Keeping::Install {
            what: Hostable::Expiring,
            forms: Vec::new(),
        },
        Keeping::Remove {
            what: Hostable::Expiring,
        },
    ] {
        assert!(dispatch(Command::Hosting(asked), &bare())
            .await
            .is_err_and(|problem| problem.code.as_str() == "HOST-1"));
    }
}

/// A guard installed against nothing is refused before anything is written.
#[tokio::test]
async fn a_guard_with_nothing_to_guard_is_refused_and_installs_nothing() {
    let manager = Fake::with(Manager::Launchd);
    let refused = dispatch(
        Command::Hosting(Keeping::Install {
            what: Hostable::Watch,
            forms: Vec::new(),
        }),
        &ctx(Arc::clone(&manager)),
    )
    .await;

    assert!(refused.is_err_and(|problem| problem.code.as_str() == "HOST-6"));
    assert!(manager.placed().is_empty());
}
