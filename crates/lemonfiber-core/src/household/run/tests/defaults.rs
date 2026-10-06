//! What somebody invited with the household's defaults would be told.

use super::*;
use crate::app::Whom;
use crate::model::MemberStanding;

/// What the request service holds for the household: nothing arrives unseen, and five
/// films a week.
const FIVE_A_WEEK: &str =
    r#"{"defaultPermissions":32,"defaultQuotas":{"movie":{"quotaLimit":5,"quotaDays":7}}}"#;

/// A household whose request service answers, and whose media server would name
/// everybody in it if anybody asked.
fn answering(main: &'static str, refuse: bool) -> Arc<Transport> {
    Transport::by_path(vec![
        (
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        ("/Users", Answer::reply(200, Fake::default().accounts)),
        (
            "/auth/me",
            Answer::reply(if refuse { 403 } else { 200 }, ""),
        ),
        ("/settings/main", Answer::reply(200, main)),
        ("", Answer::reply(200, "[]")),
    ])
}

/// The defaults are told what the household's own setting allows, as a member who has
/// asked for nothing yet: all of a period left, under no name.
#[tokio::test]
async fn the_defaults_are_told_what_the_household_allows() {
    let context = ctx_over(answering(FIVE_A_WEEK, false), "defaults-told", false);

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
    let transport = answering(FIVE_A_WEEK, false);
    let context = ctx_over(Arc::clone(&transport), "defaults-nobody", false);

    let _ = household(&context, Some(&Whom::Defaults)).await;

    let asked: Vec<String> = transport
        .requests()
        .into_iter()
        .map(|one| one.url)
        .collect();
    assert!(
        asked.iter().any(|url| url.contains("/settings/main")),
        "the household's own setting was never asked: {asked:?}"
    );
    for nobody in ["/api/v1/request", "/quota", "/user/"] {
        assert!(
            !asked.iter().any(|url| url.contains(nobody)),
            "{nobody} was read for the defaults: {asked:?}"
        );
    }
    assert!(
        !asked
            .iter()
            .any(|url| url.contains("/Users") && !url.contains("/Users/AuthenticateByName")),
        "an account was read for the defaults: {asked:?}"
    );
}

/// A request service that will not answer leaves the defaults with no policy, said as
/// unread rather than as unlimited — and with nothing to hand over.
#[tokio::test]
async fn a_request_service_that_will_not_answer_leaves_the_defaults_unread() {
    let context = ctx_over(answering(FIVE_A_WEEK, true), "defaults-refused", false);

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
    let mut context = ctx_over(answering(FIVE_A_WEEK, false), "defaults-badstack", false);
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
