use super::{
    hosting, keeping, settled, typed, Ctx, Held, Hostable, Hosting, Keeping, Standing, HOSTABLE,
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
        .with_hosting(manager as Arc<dyn Host>)
}

/// A machine that knows where it is, with this manager.
fn a_machine(manager: Arc<Fake>) -> Ctx {
    machine(knowing(), manager)
}

/// The same machine, keeping its records in an emptied scratch directory.
///
/// Apart from [`a_machine`] because almost nothing here needs it: what a reading
/// says is the manager's business and the records are not in it. The two tests
/// below are the exception — the answer to the autostart question is written
/// beside an install rather than reported by one, and a machine with nowhere to
/// keep it would read back the default whatever had been asked for.
fn recording(name: &str, manager: Arc<Fake>) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("hosting-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    machine(
        Settings {
            env_file: Some(dir.join(".env")),
            ..knowing()
        },
        manager,
    )
}

/// What this machine has recorded about bringing the stack back after a restart.
fn wants_the_stack_back(ctx: &Ctx) -> bool {
    crate::autostart::run::load(ctx).wanted().on_boot()
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

/// Every one of them, counted off the declaration rather than off a number here.
///
/// A count written down twice is a count that drifts: the reading grew a third
/// command and this went on asserting two, which is the shape of failure this
/// test exists to catch reported as this test being out of date. Read from
/// `HOSTABLE` it cannot disagree, and a command added to the reading and left off
/// this list fails here by name rather than by arithmetic.
#[tokio::test]
async fn every_long_running_command_is_on_the_reading_whether_hosted_or_not() {
    let report = read(&a_machine(Fake::with(Manager::Launchd)), Keeping::Read).await;
    assert_eq!(report.commands.len(), HOSTABLE.len());
    for what in HOSTABLE {
        // Bound rather than called inside the message. An argument to an assertion
        // that never fails is a line nothing ever enters, which the coverage gate
        // reads as dead code — and the reading is the same value either way.
        let named = what.name();
        assert_eq!(
            standing(&report, named),
            Some(Hosting::NotHosted),
            "{named}"
        );
    }
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
    assert!(manager
        .placed()
        .first()
        .is_some_and(|one| one.arguments == vec!["household".to_owned(), "expiring".to_owned()]));
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
        Ok(crate::app::Outcome::Hosting(report)) if report.commands.len() == HOSTABLE.len()
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

/// Asking this machine for the boot start *is* the operator answering the
/// autostart question, and taking it back off is them answering it the other way.
///
/// For that one command the act is the answer: somebody who installs the thing
/// that brings their stack back after a restart has said yes, and somebody who
/// removes it has said no. A recorded answer left at yes on a machine with nothing
/// installed to carry it out is `enabled-unverified` in its purest form — and the
/// cost of that state is not the wrong word on a report, it is an operator who
/// believes their stack comes back and finds out weeks later, from a household
/// asking why nothing has downloaded since Tuesday.
#[tokio::test]
async fn asking_for_the_boot_start_is_asking_for_autostart_and_so_is_taking_it_back() {
    let ctx = recording("boot", Fake::with(Manager::Launchd));

    assert!(hosting(
        &ctx,
        Keeping::Install {
            what: Hostable::Boot,
            forms: Vec::new()
        }
    )
    .await
    .is_ok());
    assert!(
        wants_the_stack_back(&ctx),
        "installing the thing that does it is saying yes to it"
    );

    assert!(hosting(
        &ctx,
        Keeping::Remove {
            what: Hostable::Boot
        }
    )
    .await
    .is_ok());
    assert!(
        !wants_the_stack_back(&ctx),
        "and taking it back off is saying no, rather than leaving a claim behind"
    );
}

/// The guard and the clock say nothing at all about what happens at a restart.
///
/// A guard on the data location stops the stack precisely because nobody chose to,
/// and a clock on requests is not a statement about starting anything. Reading
/// either as an answer to the autostart question would leave an operator who
/// installed a guard this afternoon recorded as having asked for their stack back
/// at every reboot — or, worse the other way round, would take that answer away
/// from somebody who removed a guard and never touched the question.
#[tokio::test]
async fn the_other_two_say_nothing_about_what_happens_at_a_restart() {
    let ctx = recording("others", Fake::with(Manager::Launchd));
    let already = crate::autostart::Returning::default().answering(true);
    crate::autostart::run::save(&ctx, &already);

    assert!(hosting(
        &ctx,
        Keeping::Install {
            what: Hostable::Expiring,
            forms: Vec::new()
        }
    )
    .await
    .is_ok());
    assert!(hosting(
        &ctx,
        Keeping::Remove {
            what: Hostable::Watch
        }
    )
    .await
    .is_ok());

    assert!(
        wants_the_stack_back(&ctx),
        "neither of them is a decision about starting on boot, so neither moved it"
    );
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
