//! A sent ask for music the house now holds, read back from its curator onto the household
//! list.

use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::flows::{asked, candidate, ctx, kept, ALBUM};
use crate::app::{Command, Ctx, Outcome};
use crate::collected::store::Standing;
use crate::household::State;
use crate::model::MemberRequest;
use crate::ports::service::CandidateState;
use crate::test_support::adapted::{answered, unavailable};
use crate::test_support::collecting::at;
use crate::wishing::tests::{account, call};

fn read_back(then: Answer) -> Arc<Transport> {
    super::flows::the_house(vec![
        call("requesting", account(true)),
        (
            at("candidate"),
            None,
            vec![answered(&Some(candidate(CandidateState::MayAsk))), then],
        ),
    ])
}

async fn listed(ctx: &Ctx) -> Option<MemberRequest> {
    match crate::app::dispatch(Command::Household { member: None }, ctx).await {
        Ok(Outcome::Household(report)) => report
            .members
            .into_iter()
            .flat_map(|member| member.requests)
            .find(|request| request.id == "music-1"),
        _ => None,
    }
}

async fn sent_then_read(then: Answer, tag: &str, rehearsed: bool) -> (Ctx, Option<MemberRequest>) {
    let transport = read_back(then);
    let mut ctx = ctx(&transport, tag);
    let _ = asked(&ctx, "Ada", ALBUM).await;
    ctx.dry_run = rehearsed;
    let read = listed(&ctx).await;
    (ctx, read)
}

#[tokio::test]
async fn a_sent_ask_the_curator_now_holds_is_here_with_when_it_arrived() {
    let (ctx, read) = sent_then_read(
        answered(&Some(candidate(CandidateState::Here))),
        "arrived-here",
        false,
    )
    .await;

    assert_eq!(read.as_ref().and_then(|one| one.state), Some(State::Here));
    assert!(read.and_then(|one| one.arrived).is_some());
    assert_eq!(kept(&ctx), [("music-1".to_owned(), Standing::Here)]);
}

#[tokio::test]
async fn a_sent_ask_held_in_part_is_partly_here() {
    let (ctx, read) = sent_then_read(
        answered(&Some(candidate(CandidateState::PartlyHere))),
        "arrived-partly",
        false,
    )
    .await;

    assert_eq!(
        read.as_ref().and_then(|one| one.state),
        Some(State::PartlyHere)
    );
    assert!(read.and_then(|one| one.arrived).is_some());
    assert_eq!(kept(&ctx), [("music-1".to_owned(), Standing::PartlyHere)]);
}

#[tokio::test]
async fn an_ask_still_wanted_unread_or_rehearsed_stays_on_its_way() {
    for (then, tag, rehearsed) in [
        (
            answered(&Some(candidate(CandidateState::Asked))),
            "arrived-not-yet",
            false,
        ),
        (unavailable(), "arrived-unread", false),
        (
            answered(&Some(candidate(CandidateState::Here))),
            "arrived-rehearsed",
            true,
        ),
    ] {
        let (ctx, read) = sent_then_read(then, tag, rehearsed).await;

        assert_eq!(
            read.as_ref().and_then(|one| one.state),
            Some(State::Getting),
            "{tag}"
        );
        assert_eq!(read.and_then(|one| one.arrived), None, "{tag}");
        assert_eq!(
            kept(&ctx),
            [("music-1".to_owned(), Standing::Sent)],
            "{tag}"
        );
    }
}
