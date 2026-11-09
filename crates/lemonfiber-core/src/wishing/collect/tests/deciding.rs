//! An operator ruling on an ask for music the core keeps, through the whole command.

use std::sync::Arc;

use super::flows::{asked, candidate, ctx, curator, is, kept, sent, the_house, ALBUM};
use crate::app::{Answer, Command, Ctx, Decision, Outcome};
use crate::collected::store::Standing;
use crate::collected::{Medium, Reference};
use crate::error::codes::quota::UNREACHABLE;
use crate::ports::service::CandidateState;
use crate::test_support::adapted::{answered, unavailable};
use crate::wishing::tests::house;

const FIRST: Reference = Reference::Kept(Medium::Music, 1);

async fn decided(ctx: &Ctx, answer: Answer) -> Result<Outcome, Box<crate::error::Problem>> {
    crate::app::dispatch(
        Command::Deciding(Decision {
            request: FIRST,
            answer,
        }),
        ctx,
    )
    .await
}

fn said(outcome: &Result<Outcome, Box<crate::error::Problem>>) -> Option<String> {
    match outcome {
        Ok(Outcome::Household(report)) => report.findings.first().cloned(),
        _ => None,
    }
}

#[tokio::test]
async fn an_ask_let_through_is_sent_to_the_curator_and_said_to_be_fetched() {
    let transport = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::MayAsk))),
    )]);
    let ctx = ctx(&transport, "decided-through");
    assert!(asked(&ctx, "Ada", ALBUM).await.is_ok());

    let done = decided(&ctx, Answer::LetThrough).await;

    assert!(
        said(&done).is_some_and(|said| said.contains("approved and is being fetched")),
        "{done:?}"
    );
    assert_eq!(sent(&transport, "request"), 1);
    assert_eq!(kept(&ctx), [("music-1".to_owned(), Standing::Sent)]);
}

#[tokio::test]
async fn a_rehearsed_decision_says_what_it_would_do_and_sends_nothing() {
    let transport = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::MayAsk))),
    )]);
    let mut ctx = ctx(&transport, "decided-rehearsed");
    assert!(asked(&ctx, "Ada", ALBUM).await.is_ok());
    ctx.dry_run = true;

    let done = crate::collected::run::decided(&ctx, Medium::Music, 1, None).await;

    assert!(
        done.as_deref().is_ok_and(|said| said.contains("rehearsed")),
        "{done:?}"
    );
    assert_eq!(sent(&transport, "request"), 0);
    assert_eq!(kept(&ctx), [("music-1".to_owned(), Standing::Waiting)]);
}

#[tokio::test]
async fn a_curator_that_will_not_take_it_leaves_the_ask_waiting() {
    let transport = house(vec![
        curator(
            "candidate",
            answered(&Some(candidate(CandidateState::MayAsk))),
        ),
        curator("profiles", unavailable()),
    ]);
    let ctx = ctx(&transport, "decided-unreached");
    assert!(asked(&ctx, "Ada", ALBUM).await.is_ok());

    let refused = decided(&ctx, Answer::LetThrough).await;

    assert!(is(&refused, UNREACHABLE), "{refused:?}");
    assert_eq!(kept(&ctx), [("music-1".to_owned(), Standing::Waiting)]);
}

#[tokio::test]
async fn a_house_whose_curator_has_gone_leaves_the_ask_waiting() {
    let transport = the_house(vec![curator(
        "candidate",
        answered(&Some(candidate(CandidateState::MayAsk))),
    )]);
    let asking = ctx(&transport, "decided-gone");
    assert!(asked(&asking, "Ada", ALBUM).await.is_ok());
    let mut gone = asking.clone();
    gone.seams.engine = Arc::new(crate::test_support::Reporting::holding(
        &[],
        crate::ports::docker::Lifecycle::Running,
        crate::ports::docker::Health::Healthy,
    ));

    let refused = decided(&gone, Answer::LetThrough).await;

    assert!(is(&refused, UNREACHABLE), "{refused:?}");
    assert_eq!(kept(&asking), [("music-1".to_owned(), Standing::Waiting)]);
}
