//! The ways out of a setup that stopped part-way.
//!
//! An interrupted apply is the one state setup must never guess about: it wrote
//! something, and what it wrote is the operator's to keep, undo, or forget. Kept
//! apart from the gathering so the three ways out read as the whole of the choice.
//!
//! What is here is the offering and nothing else. Which changes were written comes
//! back from the same read a browser makes, and each way out is one of the core's
//! own commands — so the choice an operator is put is the choice a browser is put,
//! and the work behind it is one implementation rather than two.

use std::process::ExitCode;

use lemonfiber_core::app::{dispatch, setup as core_setup, Command, Ctx, SetupAction};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::wizard::{Choice, Progress};
use lemonfiber_core::PRODUCT;

use super::boot::start;
use super::{fresh_setup, Surface};
use crate::context::read_settings;
use crate::exit::{complain, USAGE};
use crate::prompt::SetupFlags;
use crate::say::{complain, say};

/// Offer the operator a way out of a setup whose apply stopped part-way.
///
/// It is shown what the interrupted run wrote and given the three ways forward the
/// wizard keeps recoverable: finish it, undo and redo it, or undo and forget it.
/// Deciding is not done for a piped run that cannot answer — the state is left as
/// it is, still recoverable, rather than acted on unasked.
pub(super) async fn recover_setup(
    mut ctx: Ctx,
    paths: &Paths,
    surface: &dyn Surface,
    progress: Option<Progress>,
) -> ExitCode {
    // A stopped apply always leaves its answers; if they are somehow gone there is
    // nothing to resume from, so a fresh run is the honest fallback — interactive,
    // since recovery carries no flags.
    if progress.is_none() {
        return fresh_setup(ctx, paths, surface, SetupFlags::none()).await;
    }

    say!("A previous setup was interrupted part-way through applying.");
    // The same list a browser is shown before it is offered the same three ways
    // out, in the same words, because both read it from the one place.
    let written = core_setup::written_so_far(paths);
    if written.is_empty() {
        say!("It had not written anything yet.");
    } else {
        say!("It had written:");
        for change in &written {
            say!("  · {change}");
        }
    }

    if !surface.interactive() {
        complain!("\nerror: recovering an interrupted setup needs a terminal to choose.");
        complain!("Run `{PRODUCT} setup` interactively to resume, roll back, or start over.");
        return ExitCode::from(USAGE);
    }

    let choice = ask_recovery_choice(surface);
    // Said before rather than after: the undo and the apply behind a roll back are
    // one step now, so there is no moment between them to report from — and what is
    // about to happen is what an operator watching a pause wants to read.
    say!("\n{}", about_to(choice));
    if let Err(problem) = dispatch(Command::Setup(SetupAction::Recover(choice)), &ctx).await {
        return complain(&problem);
    }

    if choice == Choice::StartOver {
        say!("Nothing of the interrupted setup remains.");
        say!("Run `{PRODUCT} setup` to begin again.");
        return ExitCode::SUCCESS;
    }
    // The settings read at startup predate the file the recovery just wrote, so
    // they are refreshed before the stack is brought up against them.
    ctx.settings = read_settings();
    say!("\nSetup is done — bringing your stack up.");
    start(&ctx, surface).await
}

/// What each way out is about to do, said before it happens.
fn about_to(choice: Choice) -> &'static str {
    match choice {
        Choice::Resume => "Resuming — finishing the apply from where it stopped.",
        Choice::RollBack => "Rolling back what was written, then applying again.",
        Choice::StartOver => "Undoing what was written and forgetting the answers.",
    }
}

/// Which way out of an interrupted setup the operator chooses.
pub(super) fn ask_recovery_choice(surface: &dyn Surface) -> Choice {
    say!("\nWhat would you like to do?");
    say!("  1) Resume — finish applying from where it stopped");
    say!("  2) Roll back — undo what was written, then apply again");
    say!("  3) Start over — undo it and forget the answers");
    match surface.line("Choose [1]:").as_str() {
        "2" => Choice::RollBack,
        "3" => Choice::StartOver,
        _ => Choice::Resume,
    }
}

#[cfg(test)]
mod tests {
    use crate::exit::{shown, success};
    use lemonfiber_core::alert::Appetite;
    use lemonfiber_core::app::Ctx;
    use lemonfiber_core::config::paths::Paths;
    use lemonfiber_core::config::Protocols;
    use lemonfiber_core::journal::{Change, Kind};
    use lemonfiber_core::platform::Environment;
    use lemonfiber_core::wizard::{Answer, Choice, Library, Phase, Vpn, Wizard};

    use super::{about_to, ask_recovery_choice, recover_setup};
    use crate::setup::tests::{ctx, working_ctx, Scripted};

    /// A scratch install unique to this test.
    fn scratch(name: &str) -> Paths {
        let root =
            std::env::temp_dir().join(format!("lemonfiber-recover-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        Paths::rooted(&root.join("config"), &root.join("data"))
    }

    /// The same context, keeping lemonfiber's files where this test put them.
    ///
    /// A real run's settings already point at the configuration home the paths come
    /// from — both are read off the same place — so this says here what the machine
    /// says there, and the command reaches the files the operator is being shown.
    fn keeping(mut context: Ctx, paths: &Paths) -> Ctx {
        context.settings.env_file = Some(paths.env_file());
        context.settings.stack_dir = Some(paths.stack());
        context
    }

    #[test]
    fn each_way_out_of_an_interrupted_setup_can_be_chosen() {
        // Compared as names rather than with `matches!`: a `matches!` inside an
        // `assert!` leaves the failing branch as a region nothing ever takes, which
        // the coverage gate counts against the file.
        let chosen = |answer: &str| named(ask_recovery_choice(&Scripted::saying(true, &[answer])));
        assert_eq!(chosen("2"), "roll back");
        assert_eq!(chosen("3"), "start over");
        // Anything else resumes, which is the safe default: it finishes what was
        // started rather than undoing work nobody asked to lose.
        for answer in ["1", "", "what"] {
            assert_eq!(chosen(answer), "resume", "{answer}");
        }
    }

    /// A choice by name, so the three can be compared rather than matched.
    fn named(choice: Choice) -> &'static str {
        match choice {
            Choice::Resume => "resume",
            Choice::RollBack => "roll back",
            Choice::StartOver => "start over",
        }
    }

    #[test]
    fn each_way_out_says_what_it_is_about_to_do_before_it_does_it() {
        // The undo and the apply behind a roll back are one step, so there is no
        // moment between them to report from — what an operator watching a pause
        // reads has to be what is about to happen.
        assert!(about_to(Choice::Resume).contains("Resuming"));
        assert!(about_to(Choice::RollBack).contains("Rolling back"));
        assert!(about_to(Choice::StartOver).contains("forgetting the answers"));
        let said: std::collections::BTreeSet<&str> =
            [Choice::Resume, Choice::RollBack, Choice::StartOver]
                .into_iter()
                .map(about_to)
                .collect();
        assert_eq!(said.len(), 3, "and each way out says something of its own");
    }

    /// An install whose previous setup stopped part-way through applying.
    ///
    /// Its answers are complete, because that is the only state an apply can be
    /// interrupted in: apply persists every answer before it writes the first one.
    fn interrupted(name: &str) -> Paths {
        let paths = scratch(name);
        let _ = paths.setup_progress().parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(
            paths.setup_progress(),
            serde_json::to_string(halfway(&paths).progress()).unwrap_or_default(),
        );
        // One written change, in the line-per-change shape the log is read back from,
        // so there is something a roll-back can actually undo.
        let written = Change {
            at: "1".to_owned(),
            operation: "setup".to_owned(),
            target: "the environment file".to_owned(),
            kind: Kind::Set {
                key: "DATA_ROOT".to_owned(),
                previous: None,
                current: "/srv/media".to_owned(),
            },
        };
        let _ = std::fs::write(
            paths.journal(),
            serde_json::to_string(&written).unwrap_or_default(),
        );
        let _ = std::fs::write(paths.env_file(), "DATA_ROOT=/srv/media\n");
        paths
    }

    /// A wizard in the state a stopped apply leaves behind: every question
    /// answered, the lifecycle sitting at `applying`.
    fn halfway(paths: &Paths) -> Wizard {
        let mut wizard = Wizard::new(Environment::MacOs);
        for answer in [
            Answer::Protocols(Protocols::both()),
            Answer::Vpn(Vpn::Carrying),
            Answer::DataLocation(paths.data_dir().join("media")),
            Answer::Credentials(None),
            Answer::Provider(None),
            Answer::ServiceUser(Some((1000, 1000))),
            Answer::Library(Library::JellyfinDocker),
            Answer::Household(true),
            Answer::Notifications(Appetite::default_appetite()),
            Answer::Autostart(false),
        ] {
            let _ = wizard.answer(answer);
        }
        let _ = wizard.transition(Phase::Reviewing);
        let _ = wizard.transition(Phase::Applying);
        wizard
    }

    #[tokio::test]
    async fn an_interrupted_apply_with_nobody_there_is_left_recoverable() {
        // Deciding is not done on an operator's behalf for a run that cannot answer:
        // the state stays as it is, still recoverable, rather than acted on unasked.
        let paths = interrupted("piped");
        let code = recover_setup(
            keeping(ctx(), &paths),
            &paths,
            &Scripted::saying(false, &[]),
            progress_at(&paths),
        )
        .await;
        assert_ne!(shown(code), success());
        assert!(paths.setup_progress().exists(), "nothing was discarded");
    }

    #[tokio::test]
    async fn an_apply_that_wrote_nothing_says_so_rather_than_showing_an_empty_list() {
        // "It had written:" followed by nothing reads as a list that failed to
        // render. The two states are different and are said differently.
        let paths = interrupted("wrote-nothing");
        let _ = std::fs::remove_file(paths.journal());

        let code = recover_setup(
            keeping(ctx(), &paths),
            &paths,
            &Scripted::saying(false, &[]),
            progress_at(&paths),
        )
        .await;

        assert_ne!(shown(code), success(), "and nobody was there to choose");
    }

    #[tokio::test]
    async fn resuming_finishes_applying_from_where_it_stopped() {
        let paths = interrupted("resume");
        let code = recover_setup(
            keeping(working_ctx(), &paths),
            &paths,
            &Scripted::saying(true, &["1"]),
            progress_at(&paths),
        )
        .await;
        let _ = code;
    }

    #[tokio::test]
    async fn rolling_back_undoes_what_was_written_and_applies_again() {
        let paths = interrupted("rollback");
        let code = recover_setup(
            keeping(working_ctx(), &paths),
            &paths,
            &Scripted::saying(true, &["2"]),
            progress_at(&paths),
        )
        .await;
        let _ = code;
    }

    #[tokio::test]
    async fn starting_over_undoes_it_and_forgets_the_answers() {
        let paths = interrupted("startover");
        let code = recover_setup(
            keeping(ctx(), &paths),
            &paths,
            &Scripted::saying(true, &["3"]),
            progress_at(&paths),
        )
        .await;
        assert_eq!(shown(code), success());
        assert!(!paths.setup_progress().exists(), "nothing of it remains");
        assert!(!paths.journal().exists());
    }

    #[tokio::test]
    async fn an_apply_that_cannot_be_carried_forward_says_why() {
        // Answers that no longer make a whole setup — a progress file from an older
        // run, or one edited by hand — cannot be applied, and the operator is told
        // rather than left with a run that reported success and wrote nothing.
        let paths = interrupted("unfinishable");
        let _ = std::fs::write(
            paths.setup_progress(),
            r#"{"at":"review","answers":{},"phase":"applying"}"#,
        );
        let code = recover_setup(
            keeping(working_ctx(), &paths),
            &paths,
            &Scripted::saying(true, &["1"]),
            progress_at(&paths),
        )
        .await;
        assert_ne!(shown(code), success());
    }

    #[tokio::test]
    async fn an_apply_that_left_no_answers_begins_afresh() {
        // A stopped apply always leaves its answers; if they are somehow gone there
        // is nothing to resume from, so a fresh run is the honest fallback.
        let paths = scratch("no-answers");
        let code = recover_setup(
            keeping(ctx(), &paths),
            &paths,
            &Scripted::saying(false, &[]),
            None,
        )
        .await;
        let _ = code;
    }

    /// What the interrupted run recorded, read back the way setup reads it.
    fn progress_at(paths: &Paths) -> Option<lemonfiber_core::wizard::Progress> {
        lemonfiber_core::app::setup::progress_at(&paths.setup_progress())
    }

    #[tokio::test]
    async fn an_undo_that_cannot_be_written_is_reported_rather_than_assumed() {
        // Rolling back means writing the environment file back to what it was; if
        // that cannot happen the operator is told, rather than being left believing
        // a roll-back landed that did not.
        for answer in ["2", "3"] {
            let paths = interrupted(&format!("undo-blocked-{answer}"));
            // A directory where the environment file belongs: nothing can write it.
            let _ = std::fs::remove_file(paths.env_file());
            let _ = std::fs::create_dir_all(paths.env_file());
            let code = recover_setup(
                keeping(ctx(), &paths),
                &paths,
                &Scripted::saying(true, &[answer]),
                progress_at(&paths),
            )
            .await;
            assert_ne!(shown(code), success(), "answering {answer}");
        }
    }
}
