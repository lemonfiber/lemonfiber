use super::{load, noted, save};
use crate::autostart::{Held, Returning};
use crate::model::LifecycleReport;
use crate::stack::closure::Plan;
use crate::stack::compose::Action;
use crate::test_support::a_context;

/// Where a test's scratch record lives. Naming it does not touch it.
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::named(name)
}

/// A context whose environment file is in an emptied scratch directory.
fn ctx_at(name: &str) -> crate::app::Ctx {
    let dir = scratch(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    a_context()
        .runner(std::sync::Arc::new(crate::test_support::Scripted(Ok(
            crate::test_support::spoke(""),
        ))))
        .settings(crate::config::Settings {
            env_file: Some(dir.join(".env")),
            ..crate::config::Settings::default()
        })
        .build()
}

/// A report for a run that ended the way `status` says.
fn ran(status: Option<i32>, rehearsed: bool) -> LifecycleReport {
    LifecycleReport {
        action: "up".to_owned(),
        plan: Plan {
            forms: vec!["tv".to_owned()],
            profiles: std::collections::BTreeSet::new(),
            services: Vec::new(),
            dropped: Vec::new(),
            filtered: Vec::new(),
            footprint: crate::stack::closure::Footprint::default(),
        },
        command: Vec::new(),
        rehearsed,
        status,
        services: Vec::new(),
        condition: None,
        stack_edits: Vec::new(),
        port_conflicts: Vec::new(),
        forwarding: None,
        switched: None,
        held: None,
    }
}

/// The forms an operator names, as the engine takes them.
fn named(forms: &[&str]) -> Vec<String> {
    forms.iter().map(|form| (*form).to_owned()).collect()
}

/// A machine that asked for the stack back after a restart.
fn asked(ctx: &crate::app::Ctx) {
    save(ctx, &Returning::default().answering(true));
}

#[test]
fn what_was_started_is_what_a_later_run_reads_back() {
    let ctx = ctx_at("started");
    asked(&ctx);

    noted(&ctx, &Action::Up, &named(&["films"]), &ran(Some(0), false));

    assert_eq!(
        load(&ctx).at_boot().ok().map(<[String]>::to_vec),
        Some(named(&["films"]))
    );
}

#[test]
fn a_teardown_the_operator_asked_for_is_not_undone_by_a_restart() {
    let ctx = ctx_at("stopped");
    asked(&ctx);
    noted(&ctx, &Action::Up, &named(&["films"]), &ran(Some(0), false));

    noted(
        &ctx,
        &Action::Down,
        &named(&["films"]),
        &ran(Some(0), false),
    );

    assert_eq!(load(&ctx).at_boot(), Err(Held::StoppedOnPurpose));
}

#[test]
fn stopping_named_services_is_not_the_operator_putting_the_stack_down() {
    // The distinction the whole module exists for. Stopping one container to look
    // at it is a change to what is running; it is not a decision about what this
    // machine is for, and reading it as one would leave the stack down for good.
    let ctx = ctx_at("services");
    asked(&ctx);
    noted(&ctx, &Action::Up, &named(&["films"]), &ran(Some(0), false));

    for action in [
        Action::Stop(named(&["sonarr"])),
        Action::Start(named(&["sonarr"])),
        Action::Restart(named(&["sonarr"])),
        Action::Pull,
        Action::Config,
    ] {
        noted(&ctx, &action, &named(&["tv"]), &ran(Some(0), false));
    }

    assert_eq!(
        load(&ctx).at_boot().ok().map(<[String]>::to_vec),
        Some(named(&["films"])),
        "none of those said anything about what comes back"
    );
}

#[test]
fn a_run_that_did_not_work_says_nothing_about_what_this_machine_is_for() {
    // A start that Compose refused has not started anything, and recording it
    // would have the next boot bringing back a form that never came up.
    let ctx = ctx_at("failed");
    asked(&ctx);

    noted(&ctx, &Action::Up, &named(&["films"]), &ran(Some(1), false));
    noted(&ctx, &Action::Up, &named(&["films"]), &ran(None, false));

    assert_eq!(
        load(&ctx).at_boot().ok().map(<[String]>::to_vec),
        Some(Vec::new()),
        "nothing was recorded, so every form is what would come back"
    );
}

#[test]
fn a_rehearsal_writes_nothing_at_all() {
    // Twice over, because both halves have to hold: the report says it was a
    // rehearsal, and the context says the run was one.
    let ctx = ctx_at("rehearsed");
    asked(&ctx);

    noted(&ctx, &Action::Up, &named(&["films"]), &ran(None, true));
    let rehearsing = ctx_at("rehearsed-ctx");
    asked(&rehearsing);
    let rehearsing = rehearsing.rehearsing();
    noted(
        &rehearsing,
        &Action::Up,
        &named(&["films"]),
        &ran(Some(0), false),
    );

    for read in [load(&ctx), load(&rehearsing)] {
        assert_eq!(
            read.at_boot().ok().map(<[String]>::to_vec),
            Some(Vec::new()),
            "a rehearsal left a record behind"
        );
    }
}

/// What comes back is private to the operator, because it says what they run.
///
/// Not a credential, and still nobody else's business: the forms named here are a
/// list of what somebody watches, on a machine other accounts may have logins on.
/// The write goes through the same store the settings file does, which creates
/// every file it writes owner-only from the outset rather than tightening it
/// afterwards — this asserts the outcome rather than trusting the route, because
/// a record that moved to a different writer would go world-readable silently.
#[cfg(unix)]
#[test]
fn nothing_here_is_readable_by_another_account_on_this_machine() {
    use std::os::unix::fs::PermissionsExt as _;

    let ctx = ctx_at("private");
    asked(&ctx);
    noted(&ctx, &Action::Up, &named(&["films"]), &ran(Some(0), false));

    let mode = std::fs::metadata(scratch("private").join("autostart.json"))
        .map(|kept| kept.permissions().mode() & 0o777);
    assert_eq!(mode.ok(), Some(0o600), "readable only by its owner");
}

#[test]
fn a_machine_with_nothing_configured_has_nowhere_to_keep_one() {
    // Saving is a no-op rather than an error, and loading gives the default,
    // which is a machine that was never asked to start anything.
    let ctx = a_context()
        .runner(std::sync::Arc::new(crate::test_support::Scripted(Ok(
            crate::test_support::spoke(""),
        ))))
        .settings(crate::config::Settings::default())
        .build();

    noted(&ctx, &Action::Up, &named(&["films"]), &ran(Some(0), false));

    assert_eq!(load(&ctx), Returning::default());
}
