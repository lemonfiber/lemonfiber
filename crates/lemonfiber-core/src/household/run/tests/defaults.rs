//! What somebody invited with the household's defaults would be told.

use super::*;
use crate::app::Whom;
use crate::model::MemberStanding;

/// What the request service holds for the household: nothing arrives unseen, and five
/// a week.
fn five_a_week() -> crate::ports::service::Asking {
    crate::ports::service::Asking {
        approves_own: false,
        quota: Some(crate::ports::service::Quota {
            requests: 5,
            days: 7,
        }),
    }
}

/// A household whose request service answers what it allows as `asking`, refusing
/// lemonfiber's key where `refuse`.
fn answering(asking: crate::ports::service::Asking, refuse: bool) -> Arc<Transport> {
    Fake {
        answers: if refuse {
            request_service::refusing()
        } else {
            request_service::taken()
        },
        asking: request_service::answered(&asking),
        ..Fake::default()
    }
    .transport()
}

/// The defaults are told what the household's own setting allows, as a member who has
/// asked for nothing yet: all of a period left, under no name.
#[tokio::test]
async fn the_defaults_are_told_what_the_household_allows() {
    let context = ctx_over(answering(five_a_week(), false), "defaults-told", false);

    let report = household(&context, Some(&Whom::Defaults))
        .await
        .unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert_eq!(report.allows.as_deref(), Some("5 requests a week"));
    assert_eq!(report.members.len(), 1, "{report:?}");
    let member = report.members.first().cloned().unwrap_or_default();
    assert_eq!(member.name, "");
    assert!(member.requests.is_empty(), "{member:?}");
    assert!(member.access.every_library, "{member:?}");
    assert_eq!(member.access.restriction, Restriction::Unrestricted);
    assert!(member.claimed);
    assert_eq!(member.standing, MemberStanding::Active);
    let films = member.asking.as_ref().map(|asking| &asking.films);
    assert_eq!(films.and_then(|films| films.remaining), Some(5));
    assert_eq!(films.map(|films| films.used), Some(0));
    assert!(
        member
            .to_hand_over
            .iter()
            .any(|said| said.starts_with("Your limit:")),
        "{member:?}"
    );
}

/// Reading the defaults reads nobody: no account list, no requests and nobody's count.
#[tokio::test]
async fn the_defaults_read_no_account() {
    let transport = answering(five_a_week(), false);
    let context = ctx_over(Arc::clone(&transport), "defaults-nobody", false);

    let _ = household(&context, Some(&Whom::Defaults)).await;

    let asked: Vec<String> = transport
        .requests()
        .into_iter()
        .map(|one| one.url)
        .collect();
    assert!(
        asked
            .iter()
            .any(|url| url.ends_with(request_service::intake("asking"))),
        "the household's own setting was never asked: {asked:?}"
    );
    for nobody in [
        request_service::intake("requests"),
        request_service::intake("left"),
        request_service::intake("member_for"),
        media::identity("household"),
    ] {
        assert!(
            !asked.iter().any(|url| url.ends_with(nobody)),
            "{nobody} was read for the defaults: {asked:?}"
        );
    }
}

/// A request service that will not answer leaves the defaults with no policy, said as
/// unread rather than as unlimited — and with nothing to hand over.
#[tokio::test]
async fn a_request_service_that_will_not_answer_leaves_the_defaults_unread() {
    let context = ctx_over(answering(five_a_week(), true), "defaults-refused", false);

    let report = household(&context, Some(&Whom::Defaults))
        .await
        .unwrap_or_default();

    assert_eq!(report.policy, None);
    assert!(
        report
            .findings
            .iter()
            .any(|said| said.contains("would not accept lemonfiber's key")),
        "{report:?}"
    );
    assert!(
        report
            .findings
            .iter()
            .any(|said| said.contains("reported as unread rather than as unlimited")),
        "{report:?}"
    );
    assert!(
        report
            .members
            .iter()
            .all(|member| member.asking.is_none() && member.to_hand_over.is_empty()),
        "{report:?}"
    );
}

/// A stack that cannot be read is an error, for the defaults as for everybody.
#[tokio::test]
async fn the_defaults_over_an_unreadable_stack_are_an_error() {
    let mut context = ctx_over(answering(five_a_week(), false), "defaults-badstack", false);
    context.stack = crate::stack::Source::External(std::path::Path::new("/nowhere/at/all"));

    assert!(household(&context, Some(&Whom::Defaults)).await.is_err());
}

/// Naming a member still narrows the household to them.
#[tokio::test]
async fn naming_a_member_narrows_to_them() {
    let context = ctx_with(&Fake::default(), "defaults-named");

    let report = household(&context, Some(&Whom::Named("alex".to_owned())))
        .await
        .unwrap_or_default();

    assert_eq!(
        report
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Alex"]
    );
}
