use std::sync::atomic::{AtomicU32, Ordering};

use super::{looked_up, vouched, vouches, VOUCHES_FOR};
use crate::app::Ctx;
use crate::model::{Askable, Stands};
use crate::ports::media::TitleKind;
use crate::wishing::tests::{ctx_over, house};

/// A title `id` of `kind` a page offered.
fn offered(id: &str, kind: TitleKind) -> Askable {
    Askable {
        id: id.to_owned(),
        kind,
        unit: None,
        title: "Harbour Lights".to_owned(),
        by: None,
        year: None,
        cover: None,
        stands: Stands::Askable,
        found: None,
    }
}

/// The token Cy's search hands the series `1399` under `ctx`.
fn token(ctx: &Ctx) -> Option<String> {
    let mut titles = [offered("1399", TitleKind::Tv)];
    vouched(ctx, "c9d5", &mut titles);
    titles.into_iter().next().and_then(|title| title.found)
}

#[test]
fn a_token_vouches_for_the_member_kind_and_title_it_was_handed_for_and_nothing_else() {
    let ctx = ctx_over(house(Vec::new()), "found-vouches");
    let token = token(&ctx);
    let token = token.as_deref();

    assert!(token.is_some());
    assert!(vouches(&ctx, token, "c9d5", TitleKind::Tv, "1399"));
    assert!(!vouches(&ctx, token, "a7f3", TitleKind::Tv, "1399"));
    assert!(!vouches(&ctx, token, "c9d5", TitleKind::Movies, "1399"));
    assert!(!vouches(&ctx, token, "c9d5", TitleKind::Tv, "1400"));
    assert!(!vouches(&ctx, None, "c9d5", TitleKind::Tv, "1399"));
    assert!(!vouches(
        &ctx,
        Some("no-token"),
        "c9d5",
        TitleKind::Tv,
        "1399"
    ));
    assert!(!vouches(
        &ctx,
        Some("soon.abc"),
        "c9d5",
        TitleKind::Tv,
        "1399"
    ));
}

#[test]
fn a_token_whose_lapse_was_altered_or_has_passed_vouches_for_nothing() {
    let ctx = ctx_over(house(Vec::new()), "found-lapsed");
    let token = token(&ctx).unwrap_or_default();
    let (lapses, signed) = token.split_once('.').unwrap_or_default();
    let later = lapses.parse::<u64>().unwrap_or_default() + 1;
    let mut flipped = signed.to_owned();
    flipped.replace_range(..1, if signed.starts_with('0') { "1" } else { "0" });
    let stale = ctx.stamp().later(VOUCHES_FOR).seconds();
    let mut past = ctx.clone();
    past.seams.clock = lemonfiber_fixtures::ports::Stopped::at(stale + 1);

    assert!(!vouches(
        &ctx,
        Some(&format!("{later}.{signed}")),
        "c9d5",
        TitleKind::Tv,
        "1399"
    ));
    assert!(!vouches(
        &ctx,
        Some(&format!("{lapses}.{flipped}")),
        "c9d5",
        TitleKind::Tv,
        "1399"
    ));
    assert!(!vouches(&past, Some(&token), "c9d5", TitleKind::Tv, "1399"));
}

#[test]
fn a_house_with_nowhere_to_keep_a_key_hands_no_token_and_vouches_for_nothing() {
    let mut ctx = ctx_over(house(Vec::new()), "found-nowhere");
    let handed = token(&ctx);
    ctx.settings.env_file = None;

    assert_eq!(token(&ctx), None);
    assert!(!vouches(
        &ctx,
        handed.as_deref(),
        "c9d5",
        TitleKind::Tv,
        "1399"
    ));
}

/// What looking a title up comes to, where `first` is found first and each look again
/// finds what `again` says, counting each look again in `looked`.
async fn looked(
    vouched: bool,
    first: Result<Option<u8>, ()>,
    again: Result<Option<u8>, ()>,
    looked: &AtomicU32,
) -> Result<u8, Option<String>> {
    looked_up(vouched, first, || {
        looked.fetch_add(1, Ordering::Relaxed);
        async move { again }
    })
    .await
    .map_err(|problem| Some(problem.code.to_string()))
}

#[tokio::test(start_paused = true)]
async fn a_title_vouched_for_that_stays_unknown_is_looked_up_twice_more_and_then_a_try_again() {
    let again = AtomicU32::new(0);

    let said = looked(true, Ok(None), Ok(None), &again).await;

    assert_eq!(said, Err(Some("WISH-15".to_owned())));
    assert_eq!(again.load(Ordering::Relaxed), 2);
}

#[tokio::test(start_paused = true)]
async fn a_title_vouched_for_that_turns_up_on_a_second_look_is_found() {
    let again = AtomicU32::new(0);

    assert_eq!(looked(true, Ok(None), Ok(Some(7)), &again).await, Ok(7));
    assert_eq!(again.load(Ordering::Relaxed), 1);
}

#[tokio::test(start_paused = true)]
async fn a_title_nothing_vouches_for_is_unknown_at_once_and_a_failed_lookup_is_not_working() {
    let again = AtomicU32::new(0);

    assert_eq!(
        looked(false, Ok(None), Ok(Some(7)), &again).await,
        Err(Some("WISH-3".to_owned()))
    );
    assert_eq!(
        looked(true, Err(()), Ok(Some(7)), &again).await,
        Err(Some("WISH-8".to_owned()))
    );
    assert_eq!(looked(false, Ok(Some(4)), Ok(None), &again).await, Ok(4));
    assert_eq!(again.load(Ordering::Relaxed), 0);
}
