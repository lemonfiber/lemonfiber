//! What the request service will let the household ask for, driven through the HTTP
//! port against a fake transport.
//!
//! Driven from here rather than in-crate for the reason `seerr.rs` next door is: the
//! client speaks an async trait built on another, and a path exercised only from an
//! in-crate module is counted from the wrong copy.
//!
//! **Every fixture answers by route rather than in turn.** Two of these calls read
//! before they write and a third reads a document to write it back whole, so a queue
//! would prove only that the right number of requests went out — and the defect worth
//! catching here is a *narrow* body, which a queue cannot see at all.

use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Answer as Ruling, Chosen, Command, Ctx, Decision, Outcome};
use lemonfiber_core::asking::Policy;
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::http::{Http, Method, Request};
use lemonfiber_core::ports::service::{Approving, Asking, Quota};
use lemonfiber_core::seerr::Seerr;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Stopped;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};
use lemonfiber_ports::docker::{Health, Lifecycle};

fn seerr(fake: &Arc<Fake>) -> Seerr {
    let http: Arc<dyn Http> = fake.clone();
    Seerr::new(http, "http://127.0.0.1:5055", "seerr")
}

/// The account identifier the request service files a member under.
const MEMBER: &str = "4";

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

/// A decision is the last segment of the path, and the body is empty.
///
/// The service reads nothing but the path, which is the whole reason a reason cannot
/// travel with it.
#[tokio::test]
async fn a_decision_is_the_path_and_the_body_is_empty() {
    for (approve, said) in [(true, "approve"), (false, "decline")] {
        let fake = Fake::by_route(vec![(
            Method::Post,
            "/request/7/",
            Answer::reply(200, "{}"),
        )]);

        let ruled = seerr(&fake).decide(7, approve).await;

        assert!(ruled.is_ok(), "{ruled:?}");
        let asked = fake.requests();
        let sent = asked.first().map(|request| request.url.clone());
        assert!(
            sent.as_deref().is_some_and(|url| url.ends_with(said)),
            "{sent:?} does not end in {said}"
        );
        assert_eq!(asked.first().and_then(|request| request.body.clone()), None);
    }
}

/// A service that refuses is a refusal rather than a change nobody made.
#[tokio::test]
async fn a_service_that_refuses_is_a_refusal() {
    let fake = Fake::always(Answer::reply(500, "boom"));
    let client = seerr(&fake);

    assert!(client.asking().await.is_err());
    assert!(client.left(MEMBER).await.is_err());
    assert!(client.decide(7, true).await.is_err());
    assert!(client.set_asking(&Asking::default()).await.is_err());
    assert!(client.set_quota(MEMBER, None).await.is_err());
    assert!(client.approves_own(MEMBER, true).await.is_err());
}

/// A service that answers something unreadable is a refusal too, rather than a
/// household with no limit on it.
#[tokio::test]
async fn an_unreadable_answer_is_not_a_household_with_no_limit() {
    let fake = Fake::always(Answer::reply(200, "not json"));
    let client = seerr(&fake);

    assert!(client.asking().await.is_err());
    assert!(client.left(MEMBER).await.is_err());
    assert!(client.set_asking(&Asking::default()).await.is_err());
    assert!(client.set_quota(MEMBER, None).await.is_err());
    assert!(client.approves_own(MEMBER, true).await.is_err());
}

// ── Through the dispatcher, as every surface reaches it ──────────────────────

/// The stack this repository ships.
fn stack() -> Source {
    Source::External(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

/// A scratch environment file holding the media server's recorded password.
fn recorded_admin(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-asking-{}-{name}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        &["minted", "-earlier"].concat(),
    );
    env
}

/// A context over the shipped stack with the media server up and nothing answering.
///
/// Nothing answering is the point: what is held here is that the dispatcher reaches
/// these two commands at all, and that a service which will not speak leaves the
/// household exactly as it was rather than reporting a limit nobody set.
fn silent(name: &str) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &["jellyfin", "seerr"],
            Lifecycle::Running,
            Health::Healthy,
        )),
        Stopped::today(),
        Arc::new(lemonfiber_core::adapters::Disk),
        stack(),
        Settings {
            env_file: Some(recorded_admin(name)),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(Fake::silent())
}

/// A context over a transport that answers everything both writes ask.
///
/// The refusing case next door proves the dispatcher reaches these commands; this one
/// proves what they do when the service answers. Both are wanted from *outside* the
/// crate: the app layer is compiled twice, and a branch driven only from the in-crate
/// tests is counted as never run in the copy these binaries link.
fn answering(name: &str) -> Ctx {
    let transport = Fake::by_path(vec![
        // Ahead of `/Users`, whose text it contains: a route matched by prefix would
        // answer the sign-in with the list of accounts.
        (
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        (
            "/Library/MediaFolders",
            Answer::reply(200, r#"{"Items":[]}"#),
        ),
        ("/Localization/ParentalRatings", Answer::reply(200, "[]")),
        (
            "/Users",
            Answer::reply(
                200,
                r#"[{"Id":"a1","Name":"Alex","HasPassword":true,
                    "Policy":{"EnableAllFolders":true}}]"#,
            ),
        ),
        ("/auth/jellyfin", Answer::reply(200, "{}")),
        (
            "/settings/main",
            Answer::reply(
                200,
                r#"{"defaultPermissions":160,"defaultQuotas":{"movie":{},"tv":{}}}"#,
            ),
        ),
        (
            "/user/jellyfin/",
            Answer::reply(200, r#"{"id":4,"permissions":160}"#),
        ),
        (
            "/user/4/quota",
            Answer::reply(
                200,
                r#"{"movie":{"days":7,"limit":5,"used":1},"tv":{"days":7,"limit":0,"used":0}}"#,
            ),
        ),
        ("/request/7/", Answer::reply(200, "{}")),
        (
            "/api/v1/request",
            Answer::reply(
                200,
                r#"{"pageInfo":{"results":1},"results":[{"id":7,
                    "createdAt":"2026-08-17T21:04:09.000Z","status":1,"type":"movie",
                    "media":{"status":2,"externalServiceId":3},
                    "requestedBy":{"displayName":"Alex"}}]}"#,
            ),
        ),
        ("", Answer::reply(200, "[]")),
    ]);
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &["jellyfin", "seerr"],
            Lifecycle::Running,
            Health::Healthy,
        )),
        Stopped::today(),
        Arc::new(lemonfiber_core::adapters::Disk),
        stack(),
        Settings {
            env_file: Some(recorded_admin(name)),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(transport)
}

/// A choice that is written comes back as the household, under its own kind.
#[tokio::test]
async fn a_choice_that_is_written_answers_with_the_household() {
    let said = dispatch(
        Command::Allowing(Chosen {
            member: None,
            policy: Some(Policy::WithinALimit),
            quota: Some(Quota {
                requests: 5,
                days: 7,
            }),
        }),
        &answering("written"),
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(said.contains(r#""kind":"household""#), "{said}");
    assert!(said.contains("5 requests a week"), "{said}");
}

/// A request that is ruled on comes back the same way, and says what was done.
#[tokio::test]
async fn a_request_that_is_ruled_on_answers_with_the_household() {
    let said = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::TurnedDown {
                reason: "no room this month".to_owned(),
            },
        }),
        &answering("ruled"),
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(said.contains(r#""kind":"household""#), "{said}");
    assert!(said.contains("no room this month"), "{said}");
    assert!(said.contains("yours to pass on"), "{said}");
}

/// Both writes are reachable through the dispatcher, and both refuse rather than
/// claim a change nobody could make.
///
/// Driven from outside the crate because the app layer is compiled twice — once with
/// its in-crate tests and once as the library these binaries link — and an arm
/// exercised from only one is counted as never run in the other.
#[tokio::test]
async fn both_writes_refuse_rather_than_claiming_a_change() {
    let chosen = dispatch(
        Command::Allowing(Chosen {
            member: None,
            policy: Some(Policy::Trusted),
            quota: None,
        }),
        &silent("chosen"),
    )
    .await;
    let decided = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::LetThrough,
        }),
        &silent("decided"),
    )
    .await;

    for refused in [chosen, decided] {
        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(lemonfiber_core::asking::UNREACHABLE)
        );
    }
}

/// A choice that names a policy needing a limit, with none anywhere, is refused
/// before anything is written.
#[tokio::test]
async fn a_limit_that_was_never_named_is_refused_before_anything_is_written() {
    let fake = Fake::by_path(vec![
        (
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        ("/auth/jellyfin", Answer::reply(200, "{}")),
        (
            "/settings/main",
            Answer::reply(200, r#"{"defaultPermissions":32,"defaultQuotas":{}}"#),
        ),
        ("", Answer::reply(200, "[]")),
    ]);
    let ctx = Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &["jellyfin", "seerr"],
            Lifecycle::Running,
            Health::Healthy,
        )),
        Stopped::today(),
        Arc::new(lemonfiber_core::adapters::Disk),
        stack(),
        Settings {
            env_file: Some(recorded_admin("nolimit")),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(fake);

    let refused = dispatch(
        Command::Allowing(Chosen {
            member: None,
            policy: Some(Policy::WithinALimit),
            quota: None,
        }),
        &ctx,
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(lemonfiber_core::asking::NO_LIMIT)
    );
}

/// What a household may ask for reaches the machine-readable answer under the
/// household's own kind, because it is part of who is in the household.
#[tokio::test]
async fn what_may_be_asked_for_arrives_on_the_household_read() {
    let ctx = silent("read");

    let said = dispatch(Command::Household { member: None }, &ctx)
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json())
        .unwrap_or_default();

    assert!(said.contains(r#""kind":"household""#), "{said}");
    // Absent rather than shown as unlimited: nothing answered, and an unread policy
    // reported as a permissive one is the reading this whole view refuses to produce.
    assert!(said.contains(r#""policy":null"#), "{said}");
    assert!(said.contains(r#""allows":null"#), "{said}");
}
