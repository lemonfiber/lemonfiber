//! A member the request service holds no account for asks as its owner, and is kept.

use super::*;

use crate::collected::store::Standing;
use crate::collected::Reference;

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

/// What was kept for the member to be filed as the owner once approved, by reference.
fn waiting(ctx: &crate::app::Ctx) -> Vec<(String, Vec<u32>, Standing)> {
    crate::household::run::as_owner::kept(ctx)
        .waiting
        .iter()
        .map(|ask| (ask.named().to_string(), ask.seasons.clone(), ask.standing))
        .collect()
}

/// The ruling on the film `number` kept for approval, turned down for `reason` where one
/// is given, through the request service Ada's asks reach.
async fn ruled(
    ctx: &crate::app::Ctx,
    number: u64,
    reason: Option<&str>,
) -> Result<String, Box<crate::error::Problem>> {
    let reached = crate::wishing::reached(ctx, "Ada").await?;
    crate::household::run::as_owner::approving::decided(
        ctx,
        reached.requests.as_ref(),
        (Kind::Movies, number),
        reason,
    )
    .await
}

#[tokio::test]
async fn a_member_with_no_account_whose_asks_wait_has_the_ask_kept_for_approval_and_nothing_filed()
{
    let transport = titled(vec![
        call("requesting", answered(&None::<String>)),
        call("link_members", links_nobody()),
        household(None),
    ]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-kept-for-approval");

    let done = report(asked(&ctx, "Ada", Kind::Tv, "1399", &[]).await);

    assert_eq!(
        done.as_ref()
            .map(|report| (report.outcome, report.seasons.clone())),
        Some((Became::Waiting, vec![3]))
    );
    assert!(filed(&transport).is_empty());
    assert_eq!(
        waiting(&ctx),
        [("tv-1".to_owned(), vec![3], Standing::Waiting)]
    );
    assert!(crate::household::run::as_owner::kept(&ctx)
        .requests
        .is_empty());
}

#[tokio::test]
async fn an_ask_kept_for_approval_counts_toward_the_allowance_and_asked_again_is_already_asked() {
    let transport = titled(vec![
        call("requesting", answered(&None::<String>)),
        call("link_members", links_nobody()),
        household(Some(Quota {
            requests: 2,
            days: 7,
        })),
    ]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-kept-counted");

    let first = report(asked(&ctx, "Ada", Kind::Movies, "603", &[]).await);
    let again = report(asked(&ctx, "Ada", Kind::Movies, "603", &[]).await);
    let other = report(asked(&ctx, "Ada", Kind::Tv, "1399", &[]).await);

    assert_eq!(
        first
            .as_ref()
            .map(|report| (report.outcome, report.leaves.films)),
        Some((Became::Waiting, Some(1)))
    );
    assert_eq!(
        again
            .as_ref()
            .map(|report| (report.outcome, report.leaves.films, report.said.as_str())),
        Some((Became::Waiting, Some(1), super::super::ALREADY_ASKED))
    );
    assert_eq!(
        other.as_ref().map(|report| report.outcome),
        Some(Became::Waiting)
    );
    assert_eq!(
        waiting(&ctx)
            .into_iter()
            .map(|(reference, ..)| reference)
            .collect::<Vec<_>>(),
        ["movies-1", "tv-1"]
    );
    assert!(filed(&transport).is_empty());
}

#[tokio::test]
async fn a_season_kept_for_approval_and_asked_for_again_by_its_member_is_already_asked() {
    let transport = titled(vec![
        call("requesting", answered(&None::<String>)),
        call("link_members", links_nobody()),
        household(None),
    ]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-kept-season");

    let first = report(asked(&ctx, "Ada", Kind::Tv, "1399", &[3]).await);
    let again = report(asked(&ctx, "Ada", Kind::Tv, "1399", &[3]).await);

    assert_eq!(
        (
            first.as_ref().map(|report| report.outcome),
            again.as_ref().map(|report| report.outcome)
        ),
        (Some(Became::Waiting), Some(Became::Waiting))
    );
    assert!(filed(&transport).is_empty());
    assert_eq!(
        waiting(&ctx),
        [("tv-1".to_owned(), vec![3], Standing::Waiting)]
    );
}

/// The members on each ask kept for approval, by their id at the media server.
fn askers(ctx: &crate::app::Ctx) -> Vec<Vec<String>> {
    crate::household::run::as_owner::kept(ctx)
        .waiting
        .iter()
        .map(|ask| {
            ask.askers
                .iter()
                .map(|asker| asker.member.clone())
                .collect()
        })
        .collect()
}

/// A house where Ada and then Cy, neither holding an account, ask for the film `603`,
/// and what each was answered.
async fn both_asked(
    tag: &str,
    filing: Option<Route>,
) -> (
    Arc<Transport>,
    crate::app::Ctx,
    Option<AskedReport>,
    Option<AskedReport>,
) {
    let mut routes = vec![
        call("requesting", answered(&None::<String>)),
        call("link_members", links_nobody()),
        household(None),
    ];
    routes.extend(filing);
    let transport = titled(routes);
    let ctx = ctx_over(Arc::clone(&transport), tag);
    let ada = report(asked(&ctx, "Ada", Kind::Movies, "603", &[]).await);
    let cy = report(asked(&ctx, "Cy", Kind::Movies, "603", &[]).await);
    (transport, ctx, ada, cy)
}

#[tokio::test]
async fn a_second_member_asking_for_a_title_waiting_joins_it_and_is_told_only_that_it_waits() {
    let (transport, ctx, ada, cy) = both_asked("ask-kept-joined", None).await;

    let answered = |report: Option<AskedReport>| report.map(|report| (report.outcome, report.said));
    let waits = Some((
        Became::Waiting,
        "Asked. Whoever runs the house will say yes or no.".to_owned(),
    ));
    assert_eq!(answered(ada), waits);
    assert_eq!(answered(cy.clone()), waits);
    assert!(cy.is_some_and(|report| report.request.is_none() && !report.said.contains("Ada")));
    assert_eq!(askers(&ctx), [vec!["a7f3".to_owned(), "c9d5".to_owned()]]);
    assert_eq!(
        waiting(&ctx),
        [("movies-1".to_owned(), Vec::new(), Standing::Waiting)]
    );
    assert!(filed(&transport).is_empty());
}

#[tokio::test]
async fn approving_a_joined_ask_files_it_once_for_every_member_on_it() {
    let (transport, ctx, ..) =
        both_asked("ask-kept-joined-approved", Some(call("ask", filing(false)))).await;

    let decided = ruled(&ctx, 1, None).await;

    assert_eq!(
        decided.ok().as_deref(),
        Some("what Ada and Cy asked for was approved and is being fetched")
    );
    assert_eq!(filed(&transport), vec![the_film("603")]);
    let filed: Vec<(i64, String)> = crate::household::run::as_owner::kept(&ctx)
        .requests
        .iter()
        .map(|one| (one.request, one.member.clone()))
        .collect();
    assert_eq!(filed, [(7, "a7f3".to_owned()), (7, "c9d5".to_owned())]);
}

#[tokio::test]
async fn declining_a_joined_ask_closes_it_for_every_member_on_it() {
    let (transport, ctx, ..) = both_asked("ask-kept-joined-declined", None).await;

    let decided = ruled(&ctx, 1, Some("not this month")).await;

    assert!(decided.is_ok(), "{:?}", decided.err());
    assert!(filed(&transport).is_empty());
    assert_eq!(
        waiting(&ctx),
        [("movies-1".to_owned(), Vec::new(), Standing::Declined)]
    );
    assert_eq!(askers(&ctx), [vec!["a7f3".to_owned(), "c9d5".to_owned()]]);
}

#[tokio::test]
async fn approving_an_ask_kept_for_approval_files_it_as_the_owner_and_keeps_it_beside_the_member() {
    let transport = titled(vec![
        call("requesting", answered(&None::<String>)),
        call("link_members", links_nobody()),
        call("ask", filing(false)),
        household(None),
    ]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-kept-approved");
    let kept = report(asked(&ctx, "Ada", Kind::Movies, "603", &[]).await);

    let decided = ruled(&ctx, 1, None).await;

    assert_eq!(kept.map(|report| report.outcome), Some(Became::Waiting));
    assert_eq!(
        decided.ok().as_deref(),
        Some("what Ada asked for was approved and is being fetched")
    );
    assert_eq!(filed(&transport), vec![the_film("603")]);
    assert_eq!(
        told::<Option<String>>(&transport.requests(), "ask", "member"),
        vec![None]
    );
    assert_eq!(
        waiting(&ctx),
        [("movies-1".to_owned(), Vec::new(), Standing::Sent)]
    );
    let filed: Vec<(i64, String)> = crate::household::run::as_owner::kept(&ctx)
        .requests
        .iter()
        .map(|one| (one.request, one.member.clone()))
        .collect();
    assert_eq!(filed, [(7, "a7f3".to_owned())]);
}

#[tokio::test]
async fn declining_an_ask_kept_for_approval_files_nothing_and_keeps_the_reason() {
    let transport = titled(vec![
        call("requesting", answered(&None::<String>)),
        call("link_members", links_nobody()),
        household(None),
    ]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-kept-declined");
    let _ = asked(&ctx, "Ada", Kind::Movies, "603", &[]).await;

    let decided = ruled(&ctx, 1, Some("not this month")).await;
    let again = ruled(&ctx, 1, None).await;

    assert!(decided.is_ok(), "{:?}", decided.err());
    assert!(filed(&transport).is_empty());
    assert_eq!(
        waiting(&ctx),
        [("movies-1".to_owned(), Vec::new(), Standing::Declined)]
    );
    assert_eq!(
        crate::app::refusals::load(&ctx)
            .of(Reference::AsOwner(Kind::Movies, 1))
            .map(|refused| refused.reason.clone()),
        Some("not this month".to_owned())
    );
    assert_eq!(
        again.err().map(|problem| problem.code),
        Some(crate::error::codes::quota::NOT_WAITING)
    );
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

#[tokio::test]
async fn an_ask_for_approval_that_cannot_be_kept_is_not_working() {
    let transport = titled(vec![
        call("requesting", answered(&None::<String>)),
        call("link_members", links_nobody()),
        household(None),
    ]);
    let ctx = ctx_over(Arc::clone(&transport), "ask-kept-unwritable");
    if let Some(path) = crate::app::targets::beside_env(&ctx, crate::config::paths::ASKED_AS_OWNER)
    {
        let _ = std::fs::create_dir_all(path);
    }

    let refused = asked(&ctx, "Ada", Kind::Tv, "1399", &[]).await;

    assert!(refused.is_err());
    assert!(filed(&transport).is_empty());
}
