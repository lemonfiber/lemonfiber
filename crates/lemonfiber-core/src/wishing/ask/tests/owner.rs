//! A member the request service holds no account for asks as its owner, and is kept.

use super::*;

/// The request service answering that it links nobody by their id at the media server.
fn links_nobody() -> Answer {
    crate::test_support::adapted::refused(lemonfiber_contract::wire::Kind::Unsupported)
}

/// What the household may ask for where nobody chose otherwise: asks that arrive unseen,
/// three a week.
fn trusted() -> Route {
    call(
        "asking",
        answered(&Asking {
            approves_own: true,
            quota: Some(Quota {
                requests: 3,
                days: 7,
            }),
        }),
    )
}

#[tokio::test]
async fn a_member_the_request_service_holds_no_account_for_is_asked_for_as_its_owner_and_kept() {
    let transport = titled(vec![
        call("requesting", answered(&None::<String>)),
        call("link_members", links_nobody()),
        call("ask", filing(false)),
        trusted(),
    ]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-as-owner");

    let done = report(asked(&ctx, "Ada", Kind::Movies, "603", &[]).await);

    assert_eq!(
        done.as_ref()
            .map(|report| (report.outcome, report.leaves.films)),
        Some((Became::Asked, Some(2)))
    );
    assert_eq!(filed(&transport), vec![the_film("603")]);
    assert_eq!(
        told::<Option<String>>(&transport.requests(), "ask", "member"),
        vec![None]
    );
    let kept = crate::household::run::as_owner::kept(&ctx);
    let filed: Vec<(i64, &str)> = kept
        .requests
        .iter()
        .map(|one| (one.request, one.member.as_str()))
        .collect();
    assert_eq!(filed, [(7, "a7f3")]);
}

#[tokio::test]
async fn what_was_asked_for_as_the_owner_counts_toward_the_members_allowance() {
    let transport = titled(vec![
        call("requesting", answered(&None::<String>)),
        call(
            "requests",
            answered(&vec![HouseholdRequest {
                id: 7,
                made: Some(moment(-HOUR)),
                member: "owner".to_owned(),
                member_id: None,
                kind: Some(Kind::Movies),
                title_id: Some("550".to_owned()),
                item: None,
                arrived: None,
                shelf_id: None,
                request_status: Some(RequestStatus::Approved),
                media_status: Some(MediaStatus::Processing),
            }]),
        ),
        trusted(),
    ]);
    let ctx = rehearsing(ctx_over(Arc::clone(&transport), "ask-as-owner-counted"));
    let ada = crate::ports::service::Member {
        id: "a7f3".to_owned(),
        name: "Ada".to_owned(),
        ..crate::ports::service::Member::default()
    };
    let kept_at = crate::app::targets::beside_env(&ctx, crate::config::paths::ASKED_AS_OWNER)
        .unwrap_or_default();
    let filing = crate::household::run::as_owner::Filing {
        request: 7,
        title: "550",
        member: &ada,
        kind: Kind::Movies,
        counts: 1,
        now: crate::instant::Instant::from_seconds(0),
        standing: &[],
        read: crate::instant::Instant::from_seconds(0),
    };
    assert!(crate::household::run::as_owner::filed(&kept_at, filing).is_ok());

    let rehearsed = report(asked(&ctx, "Ada", Kind::Movies, "603", &[]).await);

    assert_eq!(
        rehearsed.as_ref().map(|report| report.leaves.films),
        Some(Some(1))
    );
}

#[tokio::test]
async fn a_member_with_no_account_whose_asks_wait_is_told_asks_go_through_the_house() {
    let transport = titled(vec![
        call("requesting", answered(&None::<String>)),
        call("link_members", links_nobody()),
        household(None),
    ]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-through-the-house");

    let refused = asked(&ctx, "Ada", Kind::Movies, "603", &[]).await;

    assert_eq!(code(&refused).as_deref(), Some("WISH-14"));
    assert!(filed(&transport).is_empty());
    assert!(crate::household::run::as_owner::kept(&ctx)
        .requests
        .is_empty());
}

#[tokio::test]
async fn an_ask_filed_as_the_owner_that_cannot_be_kept_is_not_working() {
    let transport = titled(vec![
        call("requesting", answered(&None::<String>)),
        call("link_members", links_nobody()),
        call("ask", filing(false)),
        trusted(),
    ]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-as-owner-unkept");
    let blocked = crate::app::targets::beside_env(&ctx, crate::config::paths::ASKED_AS_OWNER)
        .is_some_and(|path| std::fs::create_dir_all(path.join("in-the-way")).is_ok());

    let refused = asked(&ctx, "Ada", Kind::Movies, "603", &[]).await;

    assert!(blocked);
    assert!(refused.is_err());
    assert_eq!(filed(&transport), vec![the_film("603")]);
}
