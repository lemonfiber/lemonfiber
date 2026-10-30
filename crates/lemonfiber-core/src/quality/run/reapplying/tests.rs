use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use super::{customised, reapplied};
use crate::app::Ctx;
use crate::model::StackEdit;
use crate::plugin::Register;
use crate::quality::{Preset, Selection};
use crate::test_support::syncing::{a_trusted_sync, minted_for_every_part};

/// A hand edit, as the sync answers it.
const AN_EDIT: &str = r#"{"path":"recyclarr.yml","on_disk":"min: 0\n","preset":"min: 25\n"}"#;

/// The preset every curator is put back to.
fn selection() -> Selection {
    Selection::everywhere(Preset::default_preset())
}

/// A machine whose register holds a sync speaking `quality.sync` beside the curators of
/// film and television, each keyed, rehearsing where `dry_run`, answering as `http` does.
fn machine(name: &str, http: Arc<Fake>, dry_run: bool) -> Ctx {
    let scratch = lemonfiber_fixtures::scratch::Scratch::new(name);
    let project = scratch.path().to_path_buf();
    std::mem::forget(scratch);
    let mut ctx = crate::test_support::contracted_context(&project, "sync", true)
        .over(crate::test_support::embedded())
        .build()
        .with_http(http);
    ctx.settings.stack_dir = Some(project.clone());
    ctx.settings.env_file = Some(project.join(".env"));
    ctx.dry_run = dry_run;
    let directory = crate::app::project::directory(&ctx).unwrap_or_default();
    let key = crate::plugin::key_file(&directory, "sync");
    let _ = std::fs::create_dir_all(key.parent().unwrap_or(&key));
    let _ = std::fs::write(&key, crate::test_support::CONTRACTED_KEY);
    let mut register = Register::empty();
    let _ = register.record(a_trusted_sync());
    for curator in crate::test_support::curators::ALL {
        if let Some(installed) = curator.installed() {
            let _ = register.record(installed);
        }
    }
    let _ = std::fs::write(
        project.join(crate::config::paths::PLUGINS),
        register.to_json().unwrap_or_default(),
    );
    minted_for_every_part(&ctx, "the-key");
    ctx
}

#[tokio::test]
async fn a_hand_edit_is_shown_and_put_back_through_a_sync_speaking_its_contract() {
    let http = Fake::by_path(vec![
        ("/edited", Answer::reply(200, AN_EDIT)),
        (
            "/reapply",
            Answer::reply(200, r#"{"replaced":null,"applied":null}"#),
        ),
    ]);
    let rehearsing = machine("reapplying-shown", Arc::clone(&http), true);

    assert_eq!(
        reapplied(&rehearsing, &selection()).await.ok(),
        Some(Some(StackEdit {
            path: "recyclarr.yml".to_owned(),
            diff: "- min: 0\n+ min: 25\n".to_owned(),
        }))
    );
    assert!(customised(&rehearsing, &selection()).await);

    let acting = machine("reapplying-put-back", http, false);
    assert_eq!(reapplied(&acting, &selection()).await.ok(), Some(None));
}

#[tokio::test]
async fn a_sync_that_will_not_answer_is_refused_by_its_code_and_reads_as_unedited() {
    let ctx = machine(
        "reapplying-refused",
        Fake::by_path(vec![("/reapply", Answer::reply(500, ""))]),
        false,
    );

    let refused = reapplied(&ctx, &selection()).await.err();

    assert_eq!(
        refused.as_ref().map(|problem| problem.code.to_string()),
        Some("QUAL-4".to_owned())
    );
    assert!(refused.is_some_and(|problem| problem
        .summary
        .starts_with("The quality sync did not put the preset back: ")));
    assert!(!customised(&ctx, &selection()).await);
}

#[tokio::test]
async fn a_configuration_the_operator_declared_their_own_is_never_put_back() {
    let http = Fake::by_path(vec![("/reapply", Answer::reply(500, ""))]);
    let mut ctx = machine("reapplying-unmanaged", Arc::clone(&http), false);
    ctx.settings.unmanaged = vec![("sync".to_owned(), "mine".to_owned())];

    assert_eq!(reapplied(&ctx, &selection()).await.ok(), Some(None));
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn a_key_the_sync_left_in_its_edit_reaches_neither_the_report_nor_the_journal() {
    let leaking =
        r#"{"path":"recyclarr.yml","on_disk":"note: the-key-held\n","preset":"note: balanced\n"}"#;
    let http = Fake::by_path(vec![("/edited", Answer::reply(200, leaking))]);
    let ctx = machine("reapplying-leaking", http, true);
    minted_for_every_part(&ctx, "the-key-held");

    let report = super::super::quality(&ctx, crate::app::QualityAction::Reapply).await;

    let said = report
        .map(|report| serde_json::to_string(&report).unwrap_or_default())
        .unwrap_or_default();
    assert!(said.contains("note: (set, not shown)"), "{said}");
    assert!(!said.contains("the-key-held"), "{said}");
    let journal = crate::app::targets::layout(&ctx)
        .map(|paths| paths.journal())
        .and_then(|at| std::fs::read_to_string(at).ok())
        .unwrap_or_default();
    assert!(!journal.contains("the-key-held"));
}

#[tokio::test(start_paused = true)]
async fn a_sync_taking_the_profiles_up_once_it_restarts_is_restarted_once() {
    let runner = crate::test_support::egress::ran();
    let http = Fake::by_path(vec![
        (
            "/reapply",
            Answer::reply(200, r#"{"replaced":null,"applied":"on-restart"}"#),
        ),
        ("/lemonfiber/adapter/v1/ready", Answer::reply(200, "true")),
    ]);
    let mut ctx = machine("reapplying-restarted", http, false);
    ctx.seams.runner = runner.clone();

    assert_eq!(reapplied(&ctx, &selection()).await.ok(), Some(None));
    assert_eq!(crate::test_support::egress::restarts(&runner), 1);
}

#[tokio::test]
async fn an_edit_too_long_or_a_store_that_cannot_be_read_is_refused_by_its_code() {
    let long = "x".repeat(70_000);
    let answered = format!(r#"{{"path":"recyclarr.yml","on_disk":"{long}","preset":"min: 25\n"}}"#);
    let too_long = machine(
        "reapplying-too-long",
        Fake::by_path(vec![("/edited", Answer::reply(200, &answered))]),
        true,
    );
    let unread = machine(
        "reapplying-unread-store",
        Fake::by_path(vec![("/edited", Answer::reply(200, AN_EDIT))]),
        true,
    );
    let _ = std::fs::write(
        crate::settings::secrets_at(&unread).unwrap_or_default(),
        "not a store",
    );

    for ctx in [too_long, unread] {
        assert_eq!(
            reapplied(&ctx, &selection())
                .await
                .err()
                .map(|problem| problem.code.to_string()),
            Some("QUAL-4".to_owned())
        );
    }
}
