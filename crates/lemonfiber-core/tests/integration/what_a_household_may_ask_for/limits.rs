//! The household's limits and approval, read and written.

use super::common::household::answering;
use super::{seerr, MEMBER};
use lemonfiber_core::app::{dispatch, Chosen, Command};
use lemonfiber_core::asking::Policy;
use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_core::ports::service::{Asking, Quota};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::Approving;
use std::sync::Arc;

/// The household's own settings, as the service answers them.
const SETTINGS: &str = r#"{"defaultPermissions":160,
    "defaultQuotas":{"movie":{"quotaLimit":5,"quotaDays":7},
                     "tv":{"quotaLimit":5,"quotaDays":7}}}"#;

/// One member's own settings, carrying more than the quota so a narrow write shows.
const MEMBER_SETTINGS: &str = r#"{"username":"ana","email":"ana@example.test",
    "locale":"en","discoverRegion":"GB","watchlistSyncMovies":true,
    "movieQuotaLimit":null,"movieQuotaDays":null,
    "tvQuotaLimit":null,"tvQuotaDays":null}"#;

/// What the request service answers about one member's counts.
const COUNTS: &str = r#"{"movie":{"days":7,"limit":5,"used":4,"remaining":1,"restricted":false},
    "tv":{"days":7,"limit":0,"used":0,"restricted":false}}"#;

/// The body of the last request that went to a path holding `fragment`.
fn last_body_to(fake: &Arc<Fake>, fragment: &str) -> String {
    fake.requests()
        .iter()
        .filter(|request: &&Request| request.url.contains(fragment))
        .filter_map(|request| request.body.clone())
        .next_back()
        .unwrap_or_default()
}

/// The two settings that decide a policy are read as the pair they are.
#[tokio::test]
async fn the_two_settings_that_decide_a_policy_are_read_as_a_pair() {
    let fake = Fake::by_route(vec![(
        Method::Get,
        "/settings/main",
        Answer::reply(200, SETTINGS),
    )]);

    let held = seerr(&fake).asking().await;

    assert_eq!(
        held.ok(),
        Some(Asking {
            approves_own: true,
            quota: Some(Quota {
                requests: 5,
                days: 7
            }),
        })
    );
}

/// A household with a limit on neither half is a household with no limit.
#[tokio::test]
async fn a_household_with_no_limit_on_either_half_has_no_limit() {
    let fake = Fake::by_route(vec![(
        Method::Get,
        "/settings/main",
        Answer::reply(
            200,
            r#"{"defaultPermissions":32,"defaultQuotas":{"movie":{},"tv":{}}}"#,
        ),
    )]);

    let held = seerr(&fake).asking().await;

    assert_eq!(
        held.ok(),
        Some(Asking {
            approves_own: false,
            quota: None,
        })
    );
}

/// The whole-household write names the two settings and nothing else.
///
/// That write merges what it is sent into what it holds, so everything the household
/// settled elsewhere — where the media server is, what it tells them about — stays
/// settled by not being mentioned.
#[tokio::test]
async fn the_household_write_names_the_two_settings_and_nothing_else() {
    let fake = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/settings/main",
            vec![Answer::reply(200, SETTINGS)],
        ),
        (
            Method::Post,
            "/settings/main",
            vec![Answer::reply(200, SETTINGS)],
        ),
    ]);

    let written = seerr(&fake)
        .set_asking(&Asking {
            approves_own: true,
            quota: Some(Quota {
                requests: 3,
                days: 30,
            }),
        })
        .await;

    assert!(written.is_ok(), "{written:?}");
    let body = last_body_to(&fake, "/settings/main");
    assert!(body.contains(r#""quotaLimit":3"#), "{body}");
    assert!(body.contains(r#""quotaDays":30"#), "{body}");
    assert!(body.contains(r#""defaultPermissions""#), "{body}");
    assert!(
        !body.contains("hostname"),
        "the write carried more than it named"
    );
}

/// Lifting the limit writes nought rather than leaving the field out.
///
/// A field left out of a merge leaves whatever was there, so a household told nothing
/// limits it while the service goes on counting is two answers to one question.
#[tokio::test]
async fn lifting_the_household_limit_writes_nought_rather_than_nothing() {
    let fake = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/settings/main",
            vec![Answer::reply(200, SETTINGS)],
        ),
        (
            Method::Post,
            "/settings/main",
            vec![Answer::reply(200, SETTINGS)],
        ),
    ]);

    let written = seerr(&fake)
        .set_asking(&Asking {
            approves_own: true,
            quota: None,
        })
        .await;

    assert!(written.is_ok(), "{written:?}");
    let body = last_body_to(&fake, "/settings/main");
    assert!(body.contains(r#""quotaLimit":0"#), "{body}");
}

/// Taking the approval off the household writes the permissions without it.
#[tokio::test]
async fn taking_the_household_approval_off_writes_the_permissions_without_it() {
    let fake = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/settings/main",
            vec![Answer::reply(200, SETTINGS)],
        ),
        (
            Method::Post,
            "/settings/main",
            vec![Answer::reply(200, SETTINGS)],
        ),
    ]);

    let written = seerr(&fake)
        .set_asking(&Asking {
            approves_own: false,
            quota: Some(Quota {
                requests: 5,
                days: 7,
            }),
        })
        .await;

    assert!(written.is_ok(), "{written:?}");
    let body = last_body_to(&fake, "/settings/main");
    // 160 is `REQUEST` beside `AUTO_APPROVE`; without the approval it is `REQUEST`.
    assert!(body.contains(r#""defaultPermissions":32"#), "{body}");
}

/// What a member has left is the service's own arithmetic, with nought read as no
/// limit at all.
#[tokio::test]
async fn what_a_member_has_left_is_read_with_nought_as_no_limit() {
    let fake = Fake::by_route(vec![(
        Method::Get,
        "/user/4/quota",
        Answer::reply(200, COUNTS),
    )]);

    let held = seerr(&fake).left(MEMBER).await.unwrap_or_default();

    assert_eq!(held.films.limit, Some(5));
    assert_eq!(held.films.used, 4);
    assert_eq!(held.films.remaining(), Some(1));
    assert_eq!(held.television.limit, None, "nought read as a limit");
    assert!(!held.television.spent());
}

/// Setting one member's limit carries everything else about them back unchanged.
///
/// **This write assigns every field it reads off the body**, `username` and the locale
/// among them, so a body carrying only the four figures would blank a member's own name
/// on its way to setting a number. Read out of the handler in the pinned image.
#[tokio::test]
async fn setting_one_members_limit_carries_the_rest_of_them_back() {
    let fake = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/user/4/settings/main",
            vec![Answer::reply(200, MEMBER_SETTINGS)],
        ),
        (
            Method::Post,
            "/user/4/settings/main",
            vec![Answer::reply(200, MEMBER_SETTINGS)],
        ),
    ]);

    let written = seerr(&fake)
        .set_quota(
            MEMBER,
            Some(Quota {
                requests: 2,
                days: 7,
            }),
        )
        .await;

    assert!(written.is_ok(), "{written:?}");
    let body = last_body_to(&fake, "settings/main");
    assert!(body.contains(r#""username":"ana""#), "{body}");
    assert!(body.contains(r#""locale":"en""#), "{body}");
    assert!(body.contains(r#""discoverRegion":"GB""#), "{body}");
    assert!(body.contains(r#""watchlistSyncMovies":true"#), "{body}");
    assert!(body.contains(r#""movieQuotaLimit":2"#), "{body}");
    assert!(body.contains(r#""tvQuotaDays":7"#), "{body}");
}

/// Taking a member's own limit away writes nought, leaving the household's to apply.
#[tokio::test]
async fn taking_a_members_own_limit_away_writes_nought() {
    let fake = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/user/4/settings/main",
            vec![Answer::reply(200, MEMBER_SETTINGS)],
        ),
        (
            Method::Post,
            "/user/4/settings/main",
            vec![Answer::reply(200, MEMBER_SETTINGS)],
        ),
    ]);

    let written = seerr(&fake).set_quota(MEMBER, None).await;

    assert!(written.is_ok(), "{written:?}");
    let body = last_body_to(&fake, "settings/main");
    assert!(body.contains(r#""movieQuotaLimit":0"#), "{body}");
    assert!(body.contains(r#""username":"ana""#), "{body}");
}

/// Granting the approval sets one form of it and makes nobody an administrator.
#[tokio::test]
async fn granting_the_approval_makes_nobody_an_administrator() {
    let fake = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "settings/permissions",
            vec![Answer::reply(200, r#"{"permissions":32}"#)],
        ),
        (
            Method::Post,
            "settings/permissions",
            vec![Answer::reply(200, r#"{"permissions":160}"#)],
        ),
    ]);

    let written = seerr(&fake).approves_own(MEMBER, true).await;

    assert!(written.is_ok(), "{written:?}");
    let body = last_body_to(&fake, "settings/permissions");
    assert!(body.contains(r#""permissions":160"#), "{body}");
}

/// Taking it off leaves everything else about the account exactly as it was.
#[tokio::test]
async fn taking_the_approval_off_leaves_the_rest_of_the_account_alone() {
    let fake = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "settings/permissions",
            // `REQUEST`, `VOTE`, `CREATE_ISSUES` and the approval.
            vec![Answer::reply(200, r#"{"permissions":4194528}"#)],
        ),
        (
            Method::Post,
            "settings/permissions",
            vec![Answer::reply(200, r#"{"permissions":4194400}"#)],
        ),
    ]);

    let written = seerr(&fake).approves_own(MEMBER, false).await;

    assert!(written.is_ok(), "{written:?}");
    let body = last_body_to(&fake, "settings/permissions");
    assert!(body.contains(r#""permissions":4194400"#), "{body}");
}

/// A household whose settings name neither half is a household with no limit.
///
/// The service's own defaults hold `defaultQuotas` as two empty objects, and a build
/// that omitted one would be answering the same question with less. Either way it is
/// no limit rather than an answer this cannot read — which is what it was reported as
/// until this pinned it, and the two send an operator to different services.
#[tokio::test]
async fn a_settings_document_missing_a_half_is_still_no_limit() {
    for held in [
        r#"{"defaultPermissions":32,"defaultQuotas":{}}"#,
        r#"{"defaultPermissions":32,"defaultQuotas":{"movie":{}}}"#,
        r#"{"defaultPermissions":32}"#,
    ] {
        let fake = Fake::by_route(vec![(
            Method::Get,
            "/settings/main",
            Answer::reply(200, held),
        )]);

        let read = seerr(&fake).asking().await;

        assert_eq!(
            read.ok(),
            Some(Asking {
                approves_own: false,
                quota: None,
            }),
            "{held}"
        );
    }
}

/// A choice that names a policy needing a limit, with none anywhere, is refused
/// before anything is written.
#[tokio::test]
async fn a_limit_that_was_never_named_is_refused_before_anything_is_written() {
    let refused = dispatch(
        Command::Allowing(Chosen {
            member: None,
            policy: Some(Policy::WithinALimit),
            quota: None,
        }),
        // The household this answers with holds no limit on either half, so there is
        // none in force to fall back on and none was named — which is the case being
        // refused. Reached through the same fixture the writes use, because the
        // refusal has to happen after the service was asked, not instead of asking.
        &answering("nolimit"),
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(lemonfiber_core::error::codes::quota::NO_LIMIT)
    );
}
