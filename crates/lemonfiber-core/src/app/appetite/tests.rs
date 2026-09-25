use super::{alerts, record, recorded};
use crate::alert::{Appetite, Wants};
use crate::app::AlertAction;
use crate::model::AlertReport;
use crate::test_support::a_context;

/// The whole outcome as text, so a test can assert on the words in it without a
/// branch for the shape it never has.
fn said(outcome: &Result<AlertReport, Box<crate::error::Problem>>) -> String {
    format!("{outcome:?}")
}

#[tokio::test]
async fn the_preset_setup_chose_can_be_taken_again_afterwards() {
    // The whole point: setup asks once, and without this there is no second time.
    let ctx = ctx_at("revised");
    let shown = alerts(&ctx, AlertAction::Show);
    let text = said(&shown);
    assert!(text.contains("preset: \"problems-only\""), "{text}");

    let set = alerts(&ctx, AlertAction::Set(Appetite::Everything));
    let text = said(&set);
    assert!(text.contains("preset: \"everything\""), "{text}");

    // Kept, rather than reported and lost: the next run reads the same answer.
    assert_eq!(
        Wants::appetite(&recorded(&ctx)),
        Appetite::Everything,
        "the answer did not survive the call that made it"
    );
}

#[tokio::test]
async fn taking_a_preset_leaves_the_exceptions_set_apart_from_it() {
    // A broader answer is not a reason to discard the specific ones already given.
    let ctx = ctx_at("exceptions");
    let mut wants = Wants::preset(Appetite::ProblemsOnly);
    wants.set("storage.space", true);
    assert!(record(&ctx, &wants).is_ok());

    let taken = alerts(&ctx, AlertAction::Set(Appetite::Everything));
    let text = said(&taken);
    assert!(text.contains("preset: \"everything\""), "{text}");
    let kept = recorded(&ctx);
    assert_eq!(kept.exceptions().count(), 1);
}

#[tokio::test]
async fn a_rehearsal_reports_the_answer_it_would_keep_without_keeping_it() {
    let mut ctx = ctx_at("rehearsed");
    ctx.dry_run = true;
    let would = alerts(&ctx, AlertAction::Set(Appetite::Everything));
    let text = said(&would);
    assert!(text.contains("preset: \"everything\""), "{text}");
    // Reported, not written — the next run still reads the quiet default.
    assert_eq!(
        Wants::appetite(&recorded(&ctx)),
        Appetite::default_appetite()
    );
}

#[tokio::test]
async fn with_nowhere_to_keep_it_a_change_says_so_rather_than_seeming_to_work() {
    let ctx = ctx_with(None);
    assert!(alerts(&ctx, AlertAction::Set(Appetite::Everything)).is_err());
    // Reading still answers: the quiet default is a safe thing to fall back to,
    // where silently losing a change the operator made is not.
    assert!(alerts(&ctx, AlertAction::Show).is_ok());
}

/// Where a test's scratch answer lives. Naming it does not touch it.
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::named(name)
}

/// A context whose environment file is in an emptied scratch directory, so the
/// answer lands beside it and concurrent tests do not share one.
fn ctx_at(name: &str) -> crate::app::Ctx {
    let dir = scratch(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    ctx_with(Some(dir.join(".env")))
}

/// A context with the given environment file, or none at all.
fn ctx_with(env_file: Option<std::path::PathBuf>) -> crate::app::Ctx {
    let settings = crate::config::Settings {
        env_file,
        ..crate::config::Settings::default()
    };
    a_context()
        .runner(std::sync::Arc::new(crate::test_support::Scripted(Ok(
            crate::test_support::spoke(""),
        ))))
        .settings(settings)
        .build()
}

#[test]
fn the_answer_given_once_is_the_one_in_force_next_run() {
    let ctx = ctx_at("round-trip");
    let mut wants = Wants::preset(Appetite::WithCompletions);
    wants.set("update.available", true);
    assert!(record(&ctx, &wants).is_ok());
    assert_eq!(recorded(&ctx), wants);
}

#[test]
fn a_machine_nobody_has_answered_on_gets_the_quiet_default() {
    assert_eq!(recorded(&ctx_at("fresh")), Wants::default());
}

#[test]
fn an_unreadable_answer_falls_back_rather_than_refusing_the_command() {
    // Hearing less than was asked for is the safe direction; being unable to run
    // a command over a corrupt preferences file is not.
    let ctx = ctx_at("corrupt");
    assert!(record(&ctx, &Wants::preset(Appetite::Everything)).is_ok());
    let written_dir = scratch("corrupt");
    let written = written_dir.join("notifications.json");
    assert!(
        written.exists(),
        "the answer was written in the first place"
    );
    assert!(
        crate::config::store::write(&written, "not json at all").is_ok(),
        "and is then replaced with something unparsable"
    );
    assert_eq!(recorded(&ctx), Wants::default());
}

#[test]
fn an_answer_that_cannot_be_written_is_reported_rather_than_swallowed() {
    // Somewhere to keep it, and still no way to write it — a directory sits
    // where the file must go.
    let ctx = ctx_at("blocked");
    let blocked_dir = scratch("blocked");
    let blocked = blocked_dir.join("notifications.json");
    assert!(
        std::fs::create_dir_all(&blocked).is_ok(),
        "the blocking directory"
    );
    assert!(record(&ctx, &Wants::preset(Appetite::Everything)).is_err());
}

#[test]
fn an_answer_with_nowhere_to_go_is_reported_rather_than_swallowed() {
    // The operator changed something; telling them it worked when it did not is
    // worse than telling them it could not.
    let ctx = ctx_with(None);
    assert!(record(&ctx, &Wants::preset(Appetite::Everything)).is_err());
    assert_eq!(recorded(&ctx), Wants::default());
}
