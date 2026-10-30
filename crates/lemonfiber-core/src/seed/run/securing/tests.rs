use std::sync::Arc;

use lemonfiber_fixtures::http::Answer;

use super::{
    secured, secured_as, secured_by, securing_installed, would_restart, Secured, UNANSWERED,
    UNTAKEN,
};
use crate::journal::Kind;
use crate::test_support::egress::{
    answering, asked, fillers, guard, machine, ran, restarts, GUARD, PLUGIN,
};
use crate::wiring::Fillers;

#[tokio::test(start_paused = true)]
async fn a_guard_holding_its_credential_at_once_is_said_nothing_more_of() {
    let (http, runner) = (answering(Answer::reply(204, "")), ran());
    let ctx = machine("securing-now", Arc::clone(&http), Arc::clone(&runner), true);

    assert_eq!(secured(&ctx, &fillers(&ctx, &[guard()])).await, None);
    assert_eq!(asked(&http), 1);
    assert_eq!(restarts(&runner), 0);
}

#[tokio::test(start_paused = true)]
async fn a_guard_taking_it_up_once_it_restarts_is_restarted_journalled_and_said_so() {
    let (http, runner) = (answering(Answer::reply(205, "")), ran());
    let ctx = machine(
        "securing-restart",
        Arc::clone(&http),
        Arc::clone(&runner),
        true,
    );

    let came = secured(&ctx, &fillers(&ctx, &[guard()])).await;

    assert_eq!(
        came,
        Some(Secured::Restarted {
            service: GUARD.to_owned()
        })
    );
    assert_eq!(restarts(&runner), 1);
    let journalled: Vec<Kind> = crate::app::targets::layout(&ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| {
            journal
                .changes()
                .iter()
                .map(|change| change.kind.clone())
                .collect()
        })
        .unwrap_or_default();
    assert!(journalled.contains(&Kind::Restarted {
        service: GUARD.to_owned()
    }));
    assert_eq!(
        came.map(|came| came.said()).as_deref(),
        Some("egress-guard restarted to take up the control credential it was handed")
    );
}

#[tokio::test(start_paused = true)]
async fn a_guard_refusing_it_is_said_not_to_have_taken_it_up() {
    let refusing = Answer::reply(
        422,
        r#"{"code":"refused","detail":"the control server would not start"}"#,
    );
    let (http, runner) = (answering(refusing), ran());
    let ctx = machine(
        "securing-refused",
        Arc::clone(&http),
        Arc::clone(&runner),
        true,
    );

    let came = secured(&ctx, &fillers(&ctx, &[guard()])).await;

    assert!(matches!(&came, Some(Secured::Untaken { .. })), "{came:?}");
    assert!(came
        .as_ref()
        .map(Secured::said)
        .is_some_and(|said| said.starts_with(UNTAKEN)));
    assert_eq!(restarts(&runner), 0);
}

#[tokio::test(start_paused = true)]
async fn a_guard_that_does_not_answer_is_asked_again_until_its_time_is_up() {
    let http = answering(Answer::reply(204, ""));
    let mut ctx = machine("securing-unanswered", Arc::clone(&http), ran(), false);
    ctx.seams.clock = lemonfiber_fixtures::ports::Following::started();
    let until = ctx.seams.clock.now() + std::time::Duration::from_secs(2);

    let came = secured_by(&ctx, &fillers(&ctx, &[guard()]), PLUGIN, until).await;

    assert_eq!(
        came,
        Some(Secured::Untaken {
            why: UNANSWERED.to_owned()
        })
    );
    assert_eq!(asked(&http), 0);
    assert!(ctx.seams.clock.now() >= until);
}

#[tokio::test(start_paused = true)]
async fn nothing_is_asked_of_no_guard_another_plugins_guard_or_on_a_rehearsal() {
    let http = answering(Answer::reply(205, ""));
    let mut ctx = machine("securing-nothing", Arc::clone(&http), ran(), true);
    let now = ctx.seams.clock.now();
    let mut silent = guard();
    for placed in &mut silent.services {
        placed.speaks.clear();
    }

    assert_eq!(secured(&ctx, &fillers(&ctx, &[silent])).await, None);
    assert_eq!(secured(&ctx, &Fillers::default()).await, None);
    assert_eq!(
        secured_by(&ctx, &fillers(&ctx, &[guard()]), "another", now).await,
        None
    );
    ctx.dry_run = true;
    assert_eq!(secured(&ctx, &fillers(&ctx, &[guard()])).await, None);
    assert_eq!(asked(&http), 0);
}

#[tokio::test(start_paused = true)]
async fn an_installed_guard_is_asked_beside_the_register_and_an_unread_stack_asks_nothing() {
    let http = answering(Answer::reply(205, ""));
    let ctx = machine("securing-installed", Arc::clone(&http), ran(), true);
    let now = ctx.seams.clock.now();

    assert_eq!(
        securing_installed(&ctx, &guard(), now).await,
        Some(Secured::Restarted {
            service: GUARD.to_owned()
        })
    );
    let unread = crate::test_support::a_context()
        .over(crate::test_support::nowhere())
        .build();
    assert_eq!(securing_installed(&unread, &guard(), now).await, None);
    assert_eq!(asked(&http), 1);
}

#[tokio::test]
async fn a_rehearsal_says_the_guards_upstream_may_restart_and_nothing_of_anything_else() {
    let ctx = machine(
        "securing-would",
        answering(Answer::reply(205, "")),
        ran(),
        true,
    );
    let mut silent = guard();
    for placed in &mut silent.services {
        placed.speaks.clear();
    }
    let unread = crate::test_support::a_context()
        .over(crate::test_support::nowhere())
        .build();

    assert_eq!(
        would_restart(&ctx, &guard()).as_deref(),
        Some("egress-guard may restart to take up the control credential it is handed")
    );
    assert_eq!(would_restart(&ctx, &silent), None);
    assert_eq!(would_restart(&unread, &guard()), None);
}

#[test]
fn a_guard_skipped_or_offering_nothing_is_said_not_to_have_taken_it_up() {
    let untaken = |why: &str| {
        Some(Secured::Untaken {
            why: why.to_owned(),
        })
    };

    assert_eq!(
        secured_as(crate::seed::State::Skipped {
            reason: "it is not answering".to_owned()
        }),
        untaken("it is not answering")
    );
    assert_eq!(
        secured_as(crate::seed::State::Unmatched {
            reason: "it does not offer this".to_owned()
        }),
        untaken("it does not offer this")
    );
    assert_eq!(secured_as(crate::seed::State::AlreadyWired), None);
}
