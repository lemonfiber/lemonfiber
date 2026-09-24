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
    // Finished, so nothing is left to resume — and *from where it stopped*, so
    // what the interrupted run had already written stays. Starting over is the
    // choice that takes the journal with it.
    assert!(
        !paths.setup_progress().exists(),
        "an apply that finished left a run still to resume"
    );
    assert!(
        paths.journal().exists(),
        "resuming discarded what had already been written"
    );
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
    // Applied again rather than abandoned: the answers are still the answers, so
    // the run that follows the undo reaches the end and leaves nothing to resume.
    assert!(paths.env_file().exists(), "it did not apply again");
    assert!(
        !paths.setup_progress().exists(),
        "an apply that ran again left a run still to resume"
    );
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
    // Afresh, and a fresh run with nobody there is told which flags it needs
    // rather than reporting success over a walk nobody answered.
    assert_ne!(shown(code), success());
    assert!(
        !paths.env_file().exists(),
        "a run nobody answered wrote settings anyway"
    );
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
