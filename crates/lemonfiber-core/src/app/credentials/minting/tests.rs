use lemonfiber_fixtures::http::Answer;

use super::{copy, secured, Amid, GUARD_CONTROL, UNTAKEN};
use crate::credential::{Propagation, Reach};
use crate::seed::State;
use crate::test_support::egress::{answering, fillers, guard, machine, ran};

#[test]
fn a_copy_that_took_the_new_key_is_updated() {
    assert_eq!(
        copy("the gate".to_owned(), State::Wired).reach,
        Reach::Updated
    );
}

#[test]
fn a_copy_that_failed_says_why_with_nothing_credential_shaped_left_in_it() {
    let secret = ["abcdef", "1234", "567890"].concat();
    let reach = copy(
        "the gate".to_owned(),
        State::Failed {
            detail: format!("refused api_key={secret}"),
        },
    )
    .reach;

    assert!(
        matches!(&reach, Reach::Failed { detail }
            if detail.starts_with("refused") && !detail.contains(&secret)),
        "{reach:?}"
    );
}

#[test]
fn a_copy_that_neither_took_it_nor_failed_says_it_did_not_take_it() {
    let reach = copy(
        "the gate".to_owned(),
        State::Skipped {
            reason: "not yet".to_owned(),
        },
    )
    .reach;

    assert_eq!(
        reach,
        Reach::Failed {
            detail: UNTAKEN.to_owned()
        }
    );
}

#[tokio::test(start_paused = true)]
async fn a_guard_whose_credential_was_replaced_is_asked_to_hold_it_and_said_where_it_did_more() {
    let reached = |name: &'static str, answer: Answer| async move {
        let ctx = machine(name, answering(answer), ran(), true);
        let fillers = fillers(&ctx, &[guard()]);
        let amid = Amid {
            services: &[],
            fillers: &fillers,
            project: None,
        };
        secured(&ctx, &guard(), &amid).await
    };

    assert_eq!(
        reached("minting-secured-now", Answer::reply(204, "")).await,
        None
    );
    assert_eq!(
        reached("minting-secured-restart", Answer::reply(205, "")).await,
        Some(Propagation::updated(
            "egress-guard, restarted to take it up"
        ))
    );
    let refused = reached(
        "minting-secured-refused",
        Answer::reply(422, r#"{"code":"refused","detail":"no"}"#),
    )
    .await;
    assert!(
        refused.as_ref().is_some_and(
            |one| one.consumer == GUARD_CONTROL && matches!(one.reach, Reach::Failed { .. })
        ),
        "{refused:?}"
    );
}
