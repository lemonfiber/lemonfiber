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
use lemonfiber_core::config::{Reaching, Settings, REACH_HOUSEHOLD_KEY};
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

/// Where the member who asked already hears from the request service.
///
/// Both agents carrying the bit the service files a refusal under, because a member who
/// gave two addresses and asked to hear on both is the case where every arm of the
/// sending runs. The one who switched an agent off is a case of its own below.
const REACHED_AT: &str = r#"{"pushoverUserKey":"the-user-key",
    "pushoverApplicationToken":"the-application-token","pushoverSound":"bike",
    "pushbulletAccessToken":"the-access-token",
    "notificationTypes":{"pushover":64,"pushbullet":64}}"#;

/// The same member, with one of the two agents left switched off.
///
/// Nought is what the service stores for anybody who has never chosen, and it is what it
/// treats as telling them nothing on that agent.
const HALF_REACHED_AT: &str = r#"{"pushoverUserKey":"the-user-key",
    "pushoverApplicationToken":"the-application-token",
    "pushbulletAccessToken":"the-access-token",
    "notificationTypes":{"pushover":64,"pushbullet":0}}"#;

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

/// A rehearsal leaves the household's own page exactly as it found it.
///
/// The household reading now writes: the two things true of the whole house — what a
/// thing costs and whether there is room — are hung where the house will read them,
/// because there is nowhere else they could reach anybody. A reading that wrote them
/// during a rehearsal would rearrange a household's home page to answer a question
/// somebody asked without meaning to change anything.
#[tokio::test]
async fn a_rehearsal_leaves_the_households_own_page_alone() {
    let (ctx, transport) = watched("rehearsed");

    let said = dispatch(Command::Household { member: None }, &ctx.rehearsing()).await;

    assert!(said.is_ok(), "a rehearsed reading did not answer");
    let written: Vec<String> = transport
        .requests()
        .iter()
        .filter(|request: &&Request| request.url.contains("/settings/discover"))
        .map(|request| format!("{:?} {}", request.method, request.url))
        .collect();
    assert!(
        written.is_empty(),
        "a rehearsal rearranged the household's own page: {written:?}"
    );
}

/// A reading that is not a rehearsal hangs what the house is owed where they ask.
#[tokio::test]
async fn a_reading_hangs_what_the_house_is_owed_where_they_ask() {
    let (ctx, transport) = watched("hung");

    let said = dispatch(Command::Household { member: None }, &ctx).await;

    assert!(said.is_ok(), "the reading did not answer");
    let filed: Vec<String> = transport
        .requests()
        .iter()
        .filter(|request: &&Request| request.url.contains("/settings/discover/add"))
        .filter_map(|request| request.body.clone())
        .collect();
    // What is hung rather than how many: whether the disk of the machine running this
    // has room is not this test's business, and asserting a count would make it so.
    assert!(
        filed
            .iter()
            .any(|notice| notice.contains("A film about") && notice.contains("a season about")),
        "the reading hung nothing saying what a thing costs: {filed:?}"
    );
}

/// A request service that will not carry the notice costs the notice, not the reading.
///
/// The household list is what somebody typed a command to see. A page that refused to
/// hold a line for the house is worth saying out loud and worth nothing at all if the
/// price of saying it is that nobody is told who is waiting on what.
#[tokio::test]
async fn a_page_that_will_not_hold_a_notice_costs_the_notice_and_not_the_reading() {
    let said = dispatch(
        Command::Household { member: None },
        &refusing("unhung", Method::Get, "/settings/discover"),
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(
        said.contains("would not carry what the household is told"),
        "a page that refused the notice was passed over in silence: {said}"
    );
    assert!(
        said.contains("to_hand_over"),
        "a refused notice cost the reading itself: {said}"
    );
}

/// A context over a transport that answers everything both writes ask.
///
/// The refusing case next door proves the dispatcher reaches these commands; this one
/// proves what they do when the service answers. Both are wanted from *outside* the
/// crate: the app layer is compiled twice, and a branch driven only from the in-crate
/// tests is counted as never run in the copy these binaries link.
fn answering(name: &str) -> Ctx {
    with(name, Vec::new())
}

/// The same, with the transport kept so what it was sent can be read back.
///
/// Wanted for one question only — whether a rehearsal writes — and a question about
/// what did *not* go out cannot be asked of a context that swallowed its transport.
fn watched(name: &str) -> (Ctx, Arc<Fake>) {
    let transport = table(Vec::new());
    (context(name, &transport), transport)
}

/// The same, with one call answering a refusal instead.
///
/// One rule rather than a whole transport per case: every write here reaches the
/// service more than once, and what each of these holds is that the *later* calls
/// leave the household as it was — which a fixture that refused everything could not
/// tell apart from never having been asked.
fn refusing(name: &str, method: Method, route: &'static str) -> Ctx {
    with(name, vec![(Some(method), route, Answer::reply(500, "no"))])
}

/// The transport these run against, with any broken rule ahead of the working ones.
fn with(name: &str, broken: Vec<(Option<Method>, &'static str, Answer)>) -> Ctx {
    context(name, &table(broken))
}

/// The routes, with any broken rule ahead of the working ones.
fn table(broken: Vec<(Option<Method>, &'static str, Answer)>) -> Arc<Fake> {
    let mut routes = broken;
    routes.extend(vec![
        // Ahead of `/Users`, whose text it contains: a route matched by prefix would
        // answer the sign-in with the list of accounts.
        (
            None,
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        (
            None,
            "/Library/MediaFolders",
            Answer::reply(200, r#"{"Items":[]}"#),
        ),
        (
            None,
            "/Localization/ParentalRatings",
            Answer::reply(200, "[]"),
        ),
        (
            None,
            "/Users",
            Answer::reply(
                200,
                r#"[{"Id":"a1","Name":"Alex","HasPassword":true,
                    "Policy":{"EnableAllFolders":true}}]"#,
            ),
        ),
        (None, "/auth/jellyfin", Answer::reply(200, "{}")),
        (
            None,
            "/settings/main",
            Answer::reply(
                200,
                r#"{"defaultPermissions":160,"defaultQuotas":{"movie":{},"tv":{}}}"#,
            ),
        ),
        (
            None,
            "/user/jellyfin/",
            Answer::reply(200, r#"{"id":4,"permissions":160}"#),
        ),
        (
            None,
            "/user/4/quota",
            // At their limit, so the line saying so — and the sentence that says what
            // they have left and when there is room again — is built here too.
            Answer::reply(
                200,
                r#"{"movie":{"days":7,"limit":5,"used":5},"tv":{"days":7,"limit":0,"used":0}}"#,
            ),
        ),
        (
            None,
            "settings/permissions",
            Answer::reply(200, r#"{"permissions":160}"#),
        ),
        // Where the member who asked is reached. Ahead of the settings write next
        // door only in reading order; what matters is that it is behind nothing whose
        // fragment this URL also contains.
        (
            None,
            "/user/4/settings/notifications",
            Answer::reply(200, REACHED_AT),
        ),
        // The one request's own record, read to find out whose it is. Named by method
        // as well as by path, because the decision below posts to a URL this fragment
        // is a prefix of.
        (
            Some(Method::Get),
            "/request/7",
            Answer::reply(
                200,
                r#"{"id":7,"requestedBy":{"id":4,"displayName":"Alex"}}"#,
            ),
        ),
        (None, "/request/7/", Answer::reply(200, "{}")),
        (
            None,
            "/api/v1/request",
            Answer::reply(
                200,
                r#"{"pageInfo":{"results":1},"results":[{"id":7,
                    "createdAt":"2026-08-17T21:04:09.000Z","status":1,"type":"movie",
                    "media":{"status":2,"externalServiceId":3},
                    "requestedBy":{"displayName":"Alex"}}]}"#,
            ),
        ),
        (None, "", Answer::reply(200, "[]")),
    ]);
    Fake::by_rules(routes)
}

/// An install reached over the given transport.
fn context(name: &str, transport: &Arc<Fake>) -> Ctx {
    reaching(name, transport, Reaching::default())
}

/// The same, with the requests this machine allows itself said explicitly.
///
/// Wanted for one question — what a refusal comes to when the operator has switched
/// off reaching the household — and that question cannot be asked of a context whose
/// settings are always the permissive ones.
fn reaching(name: &str, transport: &Arc<Fake>, allowed: Reaching) -> Ctx {
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
            reaching: allowed,
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(transport.clone())
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
    assert!(said.contains("they have been told why"), "{said}");
}

/// The reason reaches the person who asked, where they left an address for it.
///
/// **The whole of what travels is the reason.** The request service already tells them
/// their request was declined and this must not say so again, so what goes is the word
/// `Why` and the operator's own sentence — no name of this product, no address to open,
/// nothing to sign in to. Read off what the transport was actually handed rather than off
/// the line the operator was shown, because those are different claims.
#[tokio::test]
async fn the_reason_reaches_the_person_who_asked_and_carries_nothing_else() {
    let transport = table(Vec::new());
    let said = decided(&reaching("told", &transport, Reaching::default())).await;

    assert!(
        said.contains("told why, on Pushover and Pushbullet"),
        "{said}"
    );
    assert!(!said.contains("yours to pass on"), "{said}");

    let sent: Vec<Request> = transport
        .requests()
        .into_iter()
        .filter(|asked| asked.url.contains("pushover.net"))
        .collect();
    assert_eq!(sent.len(), 1, "{sent:?}");
    let carried = sent
        .first()
        .and_then(|asked| asked.body.clone())
        .unwrap_or_default();
    assert!(
        carried.contains(r#""message":"we already have it dubbed""#),
        "{carried}"
    );
    assert!(carried.contains(r#""title":"Why""#), "{carried}");
    assert!(carried.contains(r#""user":"the-user-key""#), "{carried}");
    for absent in ["lemonfiber", "declined", "http://", "Alex"] {
        assert!(!carried.contains(absent), "{absent} travelled: {carried}");
    }
}

/// An agent the member switched off is not somewhere a message may go.
#[tokio::test]
async fn an_agent_the_member_switched_off_is_left_alone() {
    let transport = table(vec![(
        None,
        "/user/4/settings/notifications",
        Answer::reply(200, HALF_REACHED_AT),
    )]);
    let said = decided(&context("half", &transport)).await;

    assert!(said.contains("told why, on Pushover"), "{said}");
    assert!(
        !transport
            .requests()
            .iter()
            .any(|asked| asked.url.contains("pushbullet.com")),
        "an agent the member switched off was written to anyway"
    );
}

/// The words are written down as carried, so nothing can carry them a second time.
#[tokio::test]
async fn what_was_carried_is_written_down_beside_the_reason() {
    let carried = decided(&answering("told-once")).await;
    assert!(carried.contains("told why"), "{carried}");

    let read = dispatch(Command::Household { member: None }, &answering("told-once"))
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json())
        .unwrap_or_default();

    assert!(
        read.contains(r#""told":{"to":["Pushover","Pushbullet"]"#),
        "nothing records that the words went, so they could go again: {read}"
    );
}

/// An operator who switched this off is told so, and nothing leaves the machine.
#[tokio::test]
async fn a_household_this_machine_may_not_reach_is_said_rather_than_reached() {
    let transport = table(Vec::new());
    let said = decided(&reaching(
        "not-told",
        &transport,
        Reaching::without(REACH_HOUSEHOLD_KEY),
    ))
    .await;

    assert!(said.contains(REACH_HOUSEHOLD_KEY), "{said}");
    assert!(said.contains("yours to pass on"), "{said}");
    assert!(
        !transport
            .requests()
            .iter()
            .any(|asked| asked.url.contains("pushover.net")),
        "a switched-off request went anyway"
    );
}

/// A member with no address of these two kinds is an absence, not a failure.
#[tokio::test]
async fn a_member_with_nowhere_to_reach_them_is_not_a_failure() {
    let said = decided(&with(
        "no-address",
        vec![(
            None,
            "/user/4/settings/notifications",
            Answer::reply(200, "{}"),
        )],
    ))
    .await;

    assert!(said.contains("no address"), "{said}");
    assert!(said.contains("yours to pass on"), "{said}");
    assert!(said.contains("we already have it dubbed"), "{said}");
}

/// A service that would not say where they are reached leaves the words behind, and
/// says which of the two things happened.
#[tokio::test]
async fn where_they_are_reached_that_cannot_be_read_is_said_as_that() {
    let said = decided(&refusing(
        "unreadable",
        Method::Get,
        "/user/4/settings/notifications",
    ))
    .await;

    assert!(said.contains("could not be read"), "{said}");
    assert!(said.contains("yours to pass on"), "{said}");
}

/// Every address refusing names them all and leaves the words with the operator.
#[tokio::test]
async fn every_address_refusing_names_them_and_keeps_the_words_here() {
    let said = decided(&with(
        "refused",
        vec![
            (None, "pushover.net", Answer::reply(500, "no")),
            (None, "pushbullet.com", Answer::reply(500, "no")),
        ],
    ))
    .await;

    assert!(
        said.contains("Pushover and Pushbullet would not take it"),
        "{said}"
    );
    assert!(said.contains("yours to pass on"), "{said}");
}

/// One address taking it and one refusing is told once, and said as told once.
#[tokio::test]
async fn one_address_taking_it_and_one_refusing_is_told_once() {
    let said = decided(&with(
        "mixed",
        vec![(None, "pushbullet.com", Answer::reply(500, "no"))],
    ))
    .await;

    assert!(said.contains("told why, on Pushover"), "{said}");
    assert!(said.contains("Pushbullet would not take it"), "{said}");
    assert!(said.contains("once rather than twice"), "{said}");
}

/// A rehearsal decides nothing and tells nobody.
#[tokio::test]
async fn a_rehearsal_tells_nobody() {
    let transport = table(Vec::new());
    let mut ctx = reaching("rehearsed", &transport, Reaching::default());
    ctx.dry_run = true;
    let said = decided(&ctx).await;

    assert!(said.contains("nothing was sent or decided"), "{said}");
    assert!(
        !transport
            .requests()
            .iter()
            .any(|asked| asked.url.contains("pushover.net")),
        "a rehearsal told somebody"
    );
}

/// An approval carries no reason, writes nothing down and tells nobody.
#[tokio::test]
async fn an_approval_tells_nobody_because_there_is_nothing_to_tell() {
    let transport = table(Vec::new());
    let ctx = reaching("approved", &transport, Reaching::default());
    let said = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::LetThrough,
        }),
        &ctx,
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(said.contains("approved"), "{said}");
    assert!(!said.contains("told why"), "{said}");
    assert!(
        !transport
            .requests()
            .iter()
            .any(|asked| asked.url.contains("pushover.net")),
        "an approval told somebody why"
    );
}

/// One request turned down with a reason, as the answer an operator reads back.
async fn decided(ctx: &Ctx) -> String {
    dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::TurnedDown {
                reason: "we already have it dubbed".to_owned(),
            },
        }),
        ctx,
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default()
}

/// The reason a refusal carried survives it, and reaches whoever asked for the thing.
///
/// **The request service holds none.** Its endpoint reads no body and its record has no
/// column, so a reason said once on the way past would be gone by the next reading — and
/// the person told only that they were declined is in the same place as one told nothing.
/// Driven from end to end rather than asserted on the record: what matters is that the
/// words come back on the household the *next* time it is read, on a different context
/// over a different transport, which is the only proof they were written down at all.
#[tokio::test]
async fn a_reason_survives_the_refusal_and_reaches_whoever_asked() {
    let ruling = answering("passed-on");
    let decided = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::TurnedDown {
                reason: "we already have it dubbed".to_owned(),
            },
        }),
        &ruling,
    )
    .await;
    assert!(decided.is_ok(), "the refusal itself did not go through");

    // The same install read again, with the service now reporting the request as
    // refused — which is what it does once somebody has ruled on it.
    let said = dispatch(
        Command::Household { member: None },
        &with(
            "passed-on",
            vec![(
                None,
                "/api/v1/request",
                Answer::reply(
                    200,
                    r#"{"pageInfo":{"results":1},"results":[{"id":7,
                        "createdAt":"2026-08-17T21:04:09.000Z","status":3,"type":"movie",
                        "media":{"status":2,"externalServiceId":3},
                        "requestedBy":{"displayName":"Alex"}}]}"#,
                ),
            )],
        ),
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(
        said.contains(r#""reason":"we already have it dubbed""#),
        "the words were not kept: {said}"
    );
    assert!(
        said.contains("Turned down"),
        "the words were kept and not written to the person they are for: {said}"
    );
    assert!(
        said.contains("lemonfiber's own record"),
        "a reason this program holds was reported as the service's: {said}"
    );
}

/// Everything a household member is owed at the moment of asking is written to them.
///
/// The four the requirements ask for and the request service cannot show: what happens
/// to what they ask for, what their period has left and when it makes room, roughly what
/// a thing costs before they choose one, and what is still waiting on an answer.
#[tokio::test]
async fn what_a_member_is_owed_when_they_ask_is_written_to_them() {
    let said = dispatch(Command::Household { member: None }, &answering("owed"))
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json())
        .unwrap_or_default();

    assert!(said.contains("to_hand_over"), "{said}");
    assert!(said.contains("What you may ask for:"), "{said}");
    assert!(said.contains("5 of 5 a week used"), "{said}");
    assert!(said.contains("Before you ask"), "{said}");
    assert!(said.contains("Waiting on an answer:"), "{said}");
    assert!(said.contains("Nothing expires it"), "{said}");
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
        Some(lemonfiber_core::asking::NO_LIMIT)
    );
}

/// A choice about one person is written against them and read back.
///
/// The other half of the choice above, and it is a different path: it looks the member
/// up on the media server, asks the request service what they are already held to, and
/// writes against their account rather than against the household's default.
#[tokio::test]
async fn a_choice_about_one_person_is_written_and_read_back() {
    let said = dispatch(
        Command::Allowing(Chosen {
            member: Some("alex".to_owned()),
            policy: Some(Policy::Trusted),
            quota: None,
        }),
        &answering("oneperson"),
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(said.contains(r#""kind":"household""#), "{said}");
    assert!(said.contains("Alex"), "{said}");
}

/// A request let through says so, and is not asked for a reason.
///
/// The other half of the decision above. It is the half the disk can refuse, so it is
/// the one that goes past the reading of the volumes every command that brings content
/// onto the disk shares.
#[tokio::test]
async fn a_request_let_through_answers_with_the_household() {
    let said = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::LetThrough,
        }),
        &answering("letthrough"),
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(said.contains(r#""kind":"household""#), "{said}");
    assert!(said.contains("approved"), "{said}");
    assert!(!said.contains("yours to pass on"), "{said}");
}

/// A write the service will not take leaves the household as it was.
///
/// The read succeeded and the write did not, which is the case a fixture that refuses
/// everything cannot tell apart from never having asked at all.
#[tokio::test]
async fn a_write_the_service_will_not_take_changes_nothing() {
    let refused = dispatch(
        Command::Allowing(Chosen {
            member: None,
            policy: Some(Policy::Trusted),
            quota: None,
        }),
        &refusing("nowrite", Method::Post, "/settings/main"),
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(lemonfiber_core::asking::UNREACHABLE)
    );
}

/// A decision the service will not rule on says so rather than reporting it decided.
#[tokio::test]
async fn a_decision_the_service_will_not_rule_on_says_so() {
    let refused = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::LetThrough,
        }),
        &refusing("norule", Method::Post, "/request/7/"),
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(lemonfiber_core::asking::UNREACHABLE)
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
