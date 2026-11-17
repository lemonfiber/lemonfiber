//! An ask for a series or a film handing back the token the member's own search vouched
//! for it with, looked up again before it is called unknown.

use super::*;

use crate::model::{Askable, Stands};
use crate::wishing::tests::about;

/// The token Ada's search hands the film `603`, under `ctx`.
fn vouched_for_ada(ctx: &Ctx) -> Option<String> {
    let mut titles = [Askable {
        id: "603".to_owned(),
        kind: TitleKind::Movies,
        unit: None,
        title: "The Matrix".to_owned(),
        by: None,
        year: None,
        cover: None,
        stands: Stands::Askable,
        found: None,
    }];
    crate::wishing::found::vouched(ctx, "a7f3", &mut titles);
    titles.into_iter().next().and_then(|title| title.found)
}

/// What `member` asking for the film `603` with `found` comes to.
async fn asked_with(
    ctx: &Ctx,
    member: &str,
    found: Option<String>,
) -> Result<Outcome, Box<crate::error::Problem>> {
    crate::wishing::wished(
        ctx,
        crate::app::Wishing::Ask {
            member: member.to_owned(),
            wish: the_film("603"),
            found,
        },
    )
    .await
}

/// How many times the request service was asked what the film `603` is.
fn looked_up(transport: &Transport) -> usize {
    transport
        .requests()
        .iter()
        .filter(|request| {
            request.url.ends_with(intake("detail"))
                && request
                    .body
                    .as_deref()
                    .is_some_and(|body| body.contains(r#""id":"603""#))
        })
        .count()
}

/// A house whose request service answers what the film `603` is with `answers` in turn.
fn answering(answers: Vec<Answer>) -> Arc<Transport> {
    house(vec![(intake("detail"), Some(r#""id":"603""#), answers)])
}

#[tokio::test(start_paused = true)]
async fn a_film_the_members_search_just_found_that_stays_unknown_is_a_try_again_and_nothing_filed()
{
    let transport = answering(vec![answered(&None::<Detail>)]);
    let ctx = ctx_over(Arc::clone(&transport), "vouched-unknown");

    let said = asked_with(&ctx, "Ada", vouched_for_ada(&ctx)).await;

    assert_eq!(code(&said).as_deref(), Some("WISH-15"));
    assert_eq!(looked_up(&transport), 3);
    assert!(filed(&transport).is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_film_the_members_search_just_found_that_turns_up_on_a_second_look_is_asked_for() {
    let transport = answering(vec![answered(&None::<Detail>), film(MediaStatus::Unknown)]);
    let ctx = ctx_over(Arc::clone(&transport), "vouched-found");

    let said = report(asked_with(&ctx, "Ada", vouched_for_ada(&ctx)).await);

    assert!(said.is_some());
    assert_eq!(looked_up(&transport), 2);
    assert_eq!(filed(&transport), vec![the_film("603")]);
}

#[tokio::test(start_paused = true)]
async fn a_film_asked_for_with_another_members_token_or_none_is_no_such_title_at_once() {
    let transport = house(vec![about(
        "detail",
        r#""id":"603""#,
        answered(&None::<Detail>),
    )]);
    let ctx = ctx_over(Arc::clone(&transport), "vouched-theirs");
    let theirs = vouched_for_ada(&ctx);

    let borrowed = asked_with(&ctx, "Cy", theirs).await;
    let bare = asked_with(&ctx, "Ada", None).await;

    assert_eq!(code(&borrowed).as_deref(), Some("WISH-3"));
    assert_eq!(code(&bare).as_deref(), Some("WISH-3"));
    assert_eq!(looked_up(&transport), 2);
}
