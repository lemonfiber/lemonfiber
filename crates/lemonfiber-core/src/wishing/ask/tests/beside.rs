//! A member asking for what only another member's request waiting for approval asks for
//! has the ask kept beside that request, and settled once it is ruled on.

use super::*;

use crate::collected::store::Standing;
use crate::collected::Reference;
use crate::household::run::as_owner::beside::Beside;
use crate::wishing::beside::NOT_ASKED_AGAIN;

/// Ada's request `7` for the film `603`, standing as `status`.
fn adas(status: RequestStatus) -> HouseholdRequest {
    HouseholdRequest {
        id: 7,
        made: Some(moment(-HOUR)),
        member: "Ada".to_owned(),
        member_id: Some("a7f3".to_owned()),
        kind: Some(Kind::Movies),
        title_id: Some("603".to_owned()),
        item: None,
        arrived: None,
        shelf_id: None,
        request_status: Some(status),
        media_status: Some(MediaStatus::Pending),
    }
}

/// A house where Ada's request for the film `603` waits for approval, then stands as
/// `then`, answering `first` ahead of the rest.
fn waiting_then(then: RequestStatus, first: Vec<Route>) -> Arc<Transport> {
    let mut routes = first;
    routes.extend([
        titled_as("603", film(MediaStatus::Pending)),
        calls(
            "requests",
            vec![
                answered(&vec![adas(RequestStatus::Pending)]),
                answered(&vec![adas(then)]),
            ],
        ),
    ]);
    titled(routes)
}

/// Every ask kept beside another member's request.
fn beside(ctx: &crate::app::Ctx) -> Vec<Beside> {
    crate::household::run::as_owner::kept(ctx).beside
}

/// How each ask kept beside another member's request stands.
fn standings(ctx: &crate::app::Ctx) -> Vec<Standing> {
    beside(ctx).iter().map(|ask| ask.standing).collect()
}

/// Where `ctx` keeps the asks kept beside other members' requests.
fn kept_at(ctx: &crate::app::Ctx) -> Option<std::path::PathBuf> {
    crate::app::targets::beside_env(ctx, crate::config::paths::ASKED_AS_OWNER)
}

/// How many times the request service was asked for the requests it holds.
fn requests_read(transport: &Transport) -> usize {
    transport
        .requests()
        .iter()
        .filter(|request| request.url.ends_with(intake("requests")))
        .count()
}

/// Ada's request for the film `603` waiting for approval, then the request service's
/// requests answering `then`.
fn read_then(then: Answer) -> Route {
    calls(
        "requests",
        vec![answered(&vec![adas(RequestStatus::Pending)]), then],
    )
}

/// Settle the asks kept beside other members' requests, through the request service
/// Cy's asks reach.
async fn settle(ctx: &crate::app::Ctx) {
    if let Ok(reached) = crate::wishing::reached(ctx, "Cy").await {
        crate::wishing::settled_beside(ctx, reached.requests.as_ref()).await;
    }
}

#[tokio::test]
async fn a_member_asking_for_what_another_members_request_waits_on_is_told_only_that_it_waits() {
    let transport = waiting_then(RequestStatus::Pending, Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "beside-kept");

    let done = report(asked(&ctx, "Cy", Kind::Movies, "603", &[]).await);
    settle(&ctx).await;

    assert_eq!(
        done.as_ref().map(|report| report.outcome),
        Some(Became::Waiting)
    );
    assert!(done.is_some_and(|report| report
        .said
        .starts_with("Asked. Whoever runs the house will say yes or no.")
        && !report.said.contains("Ada")));
    assert!(filed(&transport).is_empty());
    assert_eq!(
        beside(&ctx)
            .iter()
            .map(|ask| (ask.requests.clone(), ask.member.as_str(), ask.standing))
            .collect::<Vec<_>>(),
        [(vec![7], "c9d5", Standing::Waiting)]
    );
}

#[tokio::test]
async fn asking_again_for_what_an_ask_kept_beside_waits_for_is_already_asked() {
    let transport = waiting_then(RequestStatus::Pending, Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "beside-again");

    let _ = asked(&ctx, "Cy", Kind::Movies, "603", &[]).await;
    let again = report(asked(&ctx, "Cy", Kind::Movies, "603", &[]).await);

    assert_eq!(
        again.map(|report| (report.outcome, report.said)),
        Some((Became::Waiting, super::super::ALREADY_ASKED.to_owned()))
    );
    assert_eq!(beside(&ctx).len(), 1);
}

#[tokio::test]
async fn approving_the_request_an_ask_waits_beside_makes_it_the_members_too() {
    let transport = waiting_then(RequestStatus::Approved, Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "beside-approved");

    let _ = asked(&ctx, "Cy", Kind::Movies, "603", &[]).await;
    settle(&ctx).await;

    assert!(filed(&transport).is_empty());
    assert_eq!(standings(&ctx), [Standing::Sent]);
    let kept = crate::household::run::as_owner::kept(&ctx);
    assert_eq!(
        kept.attributed(vec![adas(RequestStatus::Approved)])
            .iter()
            .map(|one| (one.id, one.member.as_str()))
            .collect::<Vec<_>>(),
        [(7, "Ada"), (7, "Cy")]
    );
}

#[tokio::test]
async fn declining_the_request_an_ask_waits_beside_asks_for_the_members_own() {
    let transport = waiting_then(RequestStatus::Declined, Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "beside-declined");

    let _ = asked(&ctx, "Cy", Kind::Movies, "603", &[]).await;
    settle(&ctx).await;

    assert_eq!(filed(&transport), vec![the_film("603")]);
    assert_eq!(
        told::<Option<String>>(&transport.requests(), "ask", "member"),
        vec![Some(ACCOUNT.to_owned())]
    );
    assert_eq!(standings(&ctx), [Standing::Sent]);
}

#[tokio::test]
async fn an_ask_that_cannot_be_asked_for_once_the_request_beside_is_declined_never_reads_the_reason_kept_for_that_request(
) {
    let transport = waiting_then(
        RequestStatus::Declined,
        vec![call("ask", Answer::reply(500, ""))],
    );
    let ctx = ctx_over(Arc::clone(&transport), "beside-unasked");
    let mut reasons = crate::app::refusals::load(&ctx);
    reasons.keep(Reference::Filed(7), "not this month", None);
    assert!(crate::app::refusals::keep(&ctx, &reasons).is_ok());

    let _ = asked(&ctx, "Cy", Kind::Movies, "603", &[]).await;
    settle(&ctx).await;

    let kept = beside(&ctx);
    assert_eq!(
        kept.iter()
            .map(|ask| (ask.standing, ask.why.as_deref()))
            .collect::<Vec<_>>(),
        [(Standing::Declined, Some(NOT_ASKED_AGAIN))]
    );
    let line = kept.first().map(|ask| {
        ask.line(
            &crate::quality::Selection::everywhere(crate::ports::media::preset::Preset::Balanced),
            ctx.stamp(),
        )
    });
    assert_eq!(
        line.map(|line| (
            line.id,
            line.state,
            line.refused.map(|refused| refused.reason)
        )),
        Some((
            "7".to_owned(),
            Some(crate::household::State::Declined),
            Some(NOT_ASKED_AGAIN.to_owned())
        ))
    );
}

#[tokio::test]
async fn an_ask_kept_beside_stays_waiting_while_the_requests_cannot_be_read() {
    let transport = waiting_then(RequestStatus::Pending, vec![read_then(unavailable())]);
    let ctx = ctx_over(Arc::clone(&transport), "beside-unread");

    let _ = asked(&ctx, "Cy", Kind::Movies, "603", &[]).await;
    let before = requests_read(&transport);
    settle(&ctx).await;

    assert_eq!(requests_read(&transport), before + 1);
    assert!(filed(&transport).is_empty());
    assert_eq!(standings(&ctx), [Standing::Waiting]);
}

#[tokio::test]
async fn an_ask_whose_allowance_cannot_be_read_once_the_request_beside_is_declined_reads_declined()
{
    let transport = waiting_then(
        RequestStatus::Declined,
        vec![calls(
            "requests",
            vec![
                answered(&vec![adas(RequestStatus::Pending)]),
                answered(&vec![adas(RequestStatus::Declined)]),
                unavailable(),
            ],
        )],
    );
    let ctx = ctx_over(Arc::clone(&transport), "beside-uncounted");

    let _ = asked(&ctx, "Cy", Kind::Movies, "603", &[]).await;
    settle(&ctx).await;

    assert!(filed(&transport).is_empty());
    assert_eq!(
        beside(&ctx)
            .iter()
            .map(|ask| (ask.standing, ask.why.as_deref()))
            .collect::<Vec<_>>(),
        [(Standing::Declined, Some(NOT_ASKED_AGAIN))]
    );
}

#[tokio::test]
async fn an_ask_kept_beside_that_cannot_be_written_stays_as_it_was_kept() {
    let transport = waiting_then(RequestStatus::Approved, Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "beside-locked");

    let _ = asked(&ctx, "Cy", Kind::Movies, "603", &[]).await;
    if let Some(path) = kept_at(&ctx) {
        crate::test_support::written_later(&path);
    }
    let before = requests_read(&transport);
    settle(&ctx).await;

    assert_eq!(requests_read(&transport), before + 1);
    assert_eq!(standings(&ctx), [Standing::Waiting]);
    assert!(crate::household::run::as_owner::kept(&ctx)
        .requests
        .is_empty());
}

#[tokio::test]
async fn a_house_with_nowhere_to_keep_asks_settles_none_and_reads_nothing() {
    let transport = waiting_then(RequestStatus::Approved, Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "beside-nowhere");

    let _ = asked(&ctx, "Cy", Kind::Movies, "603", &[]).await;
    let reached = crate::wishing::reached(&ctx, "Cy").await;
    assert!(reached.is_ok());
    let Ok(reached) = reached else {
        return;
    };
    let mut nowhere = ctx.clone();
    nowhere.settings.env_file = None;
    let before = requests_read(&transport);
    crate::wishing::settled_beside(&nowhere, reached.requests.as_ref()).await;

    assert_eq!(requests_read(&transport), before);
    assert_eq!(standings(&ctx), [Standing::Waiting]);
}

#[tokio::test]
async fn an_ask_that_cannot_be_kept_beside_the_request_it_waits_on_is_asking_not_working() {
    let transport = waiting_then(RequestStatus::Pending, Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "beside-unkept");
    let _ = asked(&ctx, "Cy", Kind::Movies, "603", &[]).await;
    if let Some(path) = kept_at(&ctx) {
        crate::test_support::written_later(&path);
    }

    let unkept = asked(&ctx, "Bea", Kind::Movies, "603", &[]).await;

    assert_eq!(code(&unkept).as_deref(), Some("WISH-8"));
    assert!(filed(&transport).is_empty());
    assert_eq!(beside(&ctx).len(), 1);
}

#[tokio::test]
async fn an_ask_kept_beside_a_declined_request_in_a_house_with_no_room_is_declined_as_a_fresh_ask_is(
) {
    let transport = waiting_then(
        RequestStatus::Declined,
        vec![titled_as("550", film(MediaStatus::Unknown))],
    );
    let ctx = ctx_over(Arc::clone(&transport), "beside-no-room");
    let _ = asked(&ctx, "Cy", Kind::Movies, "603", &[]).await;
    let full = without_room(ctx.clone());

    settle(&full).await;
    let fresh = asked(&full, "Bea", Kind::Movies, "550", &[]).await;

    assert!(filed(&transport).is_empty());
    assert_eq!(code(&fresh).as_deref(), Some("WISH-7"));
    assert_eq!(
        beside(&ctx)
            .iter()
            .map(|ask| (ask.standing, ask.why.clone()))
            .collect::<Vec<_>>(),
        [(
            Standing::Declined,
            fresh.err().map(|problem| problem.summary)
        )]
    );
}
