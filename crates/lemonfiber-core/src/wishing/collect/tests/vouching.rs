//! An ask for music or books handing back the token the member's own search vouched for it
//! with, looked up again before it is called unknown.

use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::flows::{candidate, ctx, curator, the_house, ALBUM};
use crate::app::{Ctx, Outcome, Wishing};
use crate::model::{Askable, Became, Stands, TitleKind};
use crate::ports::service::{Candidate, CandidateState};
use crate::test_support::adapted::answered;
use crate::test_support::collecting::at;
use crate::wishing::tests::code;

/// The token Ada's search hands the album it found, under `ctx`.
fn vouched_for_ada(ctx: &Ctx) -> Option<String> {
    let mut titles = [Askable {
        id: candidate(CandidateState::MayAsk).reference,
        kind: TitleKind::Music,
        unit: Some(ALBUM.into()),
        title: "Kind of Blue".to_owned(),
        by: None,
        year: None,
        cover: None,
        stands: Stands::Askable,
        found: None,
    }];
    crate::wishing::found::vouched(ctx, "a7f3", &mut titles);
    titles.into_iter().next().and_then(|title| title.found)
}

/// What `member` asking for the album with `found` comes to.
async fn asked_with(
    ctx: &Ctx,
    member: &str,
    found: Option<String>,
) -> Result<Outcome, Box<crate::error::Problem>> {
    crate::wishing::wished(
        ctx,
        Wishing::Collect {
            member: member.to_owned(),
            unit: ALBUM,
            reference: candidate(CandidateState::MayAsk).reference,
            found,
        },
    )
    .await
}

/// How many times the curator was asked for the candidate.
fn looked_up(transport: &Transport) -> usize {
    transport
        .requests()
        .iter()
        .filter(|request| request.url.ends_with(at("candidate")))
        .count()
}

/// A house whose curator answers the candidate `answers` in turn.
fn answering(answers: Vec<Answer>) -> Arc<Transport> {
    the_house(vec![(at("candidate"), None, answers)])
}

#[tokio::test(start_paused = true)]
async fn an_album_the_members_search_just_found_that_stays_unknown_is_a_try_again() {
    let transport = answering(vec![answered(&None::<Candidate>)]);
    let ctx = ctx(&transport, "vouched-unknown");

    let said = asked_with(&ctx, "Ada", vouched_for_ada(&ctx)).await;

    assert_eq!(code(&said).as_deref(), Some("WISH-15"));
    assert_eq!(looked_up(&transport), 3);
}

#[tokio::test(start_paused = true)]
async fn an_album_the_members_search_just_found_that_turns_up_on_a_second_look_is_asked_about() {
    let transport = answering(vec![
        answered(&None::<Candidate>),
        answered(&Some(candidate(CandidateState::Asked))),
    ]);
    let ctx = ctx(&transport, "vouched-found");

    let said = asked_with(&ctx, "Ada", vouched_for_ada(&ctx)).await;

    assert!(matches!(said, Ok(Outcome::Asked(ref report)) if report.outcome == Became::OnItsWay));
    assert_eq!(looked_up(&transport), 2);
}

#[tokio::test(start_paused = true)]
async fn an_album_asked_for_with_another_members_token_or_none_is_no_such_title_at_once() {
    let transport = the_house(vec![curator("candidate", answered(&None::<Candidate>))]);
    let ctx = ctx(&transport, "vouched-theirs");
    let theirs = vouched_for_ada(&ctx);

    let borrowed = asked_with(&ctx, "Bea", theirs).await;
    let bare = asked_with(&ctx, "Ada", None).await;

    assert_eq!(code(&borrowed).as_deref(), Some("WISH-3"));
    assert_eq!(code(&bare).as_deref(), Some("WISH-3"));
    assert_eq!(looked_up(&transport), 2);
}
