//! What this machine was last asked to run, and whether it was asked to stop.
//!
//! The record setup writes holds the operator's answer about starting on boot, and
//! an answer on its own cannot start anything: a machine that knows somebody wants
//! their stack back does not know *which* stack, and a machine that starts one
//! somebody deliberately stopped on Friday has ignored the clearest instruction it
//! was ever given. Both of those are answered here, by writing down what a lifecycle
//! command was actually asked to do.
//!
//! **Written where the operator's intent is known, which is not where the engine is
//! driven.** The guard on the data location stops the stack too, and it stops it
//! precisely because nobody chose to — so a rule that read every teardown as a
//! decision would leave a machine that unplugged a drive once and never brought its
//! stack back again. So only the two whole-stack actions are recorded, and the
//! service-by-service ones are not: stopping one container is not the operator
//! putting their stack down for the night.
//!
//! Best effort on the way out, like the conditions and the outbox beside it. A
//! record that could not be written costs the next boot its knowledge of which form
//! to bring back — which falls through to every form, the same answer naming no form
//! gives everywhere else — and that is a worse answer rather than a wrong claim, and
//! never worth failing a start over.

use crate::autostart::Returning;
use crate::model::LifecycleReport;
use crate::stack::compose::Action;

use crate::app::Ctx;

/// The record's file name, named once so the layout and the readers agree.
///
/// The same file [`crate::config::paths::Paths::autostart`] names. Setup writes the
/// answer into it before anything has ever been run, and this writes the rest of it
/// afterwards, which is why both spellings have to land on one file.
const RECORD: &str = "autostart.json";

/// What is to come back after a restart, as the last run left it.
#[must_use]
pub(crate) fn load(ctx: &Ctx) -> Returning {
    crate::app::record::beside(ctx, RECORD)
}

/// Write it where the next run — and the next boot — will read it.
pub(crate) fn save(ctx: &Ctx, returning: &Returning) {
    crate::app::record::keep_beside(ctx, RECORD, returning);
}

/// What one lifecycle action says about what this machine is for.
///
/// Two of the eight say anything at all. The rest change what is running without
/// saying anything about what should be: stopping one container to look at it,
/// fetching an image, or taking back a container an install put there and could not
/// prove, is not the operator putting their stack down for the night.
enum Said {
    /// These forms are what the operator wants running.
    Running,
    /// The stack is to stay down until they say otherwise.
    Stopped,
}

/// What this action amounts to, or nothing where it amounts to nothing.
const fn said(action: &Action) -> Option<Said> {
    match action {
        Action::Up => Some(Said::Running),
        Action::Down => Some(Said::Stopped),
        Action::Start(_)
        | Action::Stop(_)
        | Action::Remove(_)
        | Action::Restart(_)
        | Action::Pull
        | Action::Config => None,
    }
}

/// Write down what this lifecycle command was asked to do, where it did it.
///
/// Called from both ways of running one. A start can be waited on or streamed, and
/// the streamed one is what an operator at a terminal actually uses — so a record
/// written on only the waited-on path is a record that is right in the tests and
/// empty on every real machine.
pub(crate) fn noted(ctx: &Ctx, action: &Action, forms: &[String], report: &LifecycleReport) {
    if !carried(ctx, report) {
        return;
    }
    let Some(said) = said(action) else {
        return;
    };
    let mut returning = load(ctx);
    match said {
        Said::Running => returning.started(forms),
        Said::Stopped => returning.stopped(),
    }
    save(ctx, &returning);
}

/// Whether this run actually did the thing it was asked to do.
///
/// A rehearsal is excluded first and by itself, because it is the one case where the
/// report describes something that did not happen: a rehearsal reports the plan it
/// would have run, and a record written from it would say the stack was started by a
/// command whose whole promise is that it touched nothing.
fn carried(ctx: &Ctx, report: &LifecycleReport) -> bool {
    !ctx.dry_run && !report.rehearsed && report.status == Some(0)
}

#[cfg(test)]
mod tests {
    use super::{load, noted, save};
    use crate::autostart::{Held, Returning};
    use crate::model::LifecycleReport;
    use crate::stack::closure::Plan;
    use crate::stack::compose::Action;
    use crate::test_support::a_context;

    /// Where a test's scratch record lives. Naming it does not touch it.
    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "lemonfiber-returning-{}-{name}",
            std::process::id()
        ))
    }

    /// A context whose environment file is in an emptied scratch directory.
    fn ctx_at(name: &str) -> crate::app::Ctx {
        let dir = scratch(name);
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
}
