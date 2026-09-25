use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{agreeing, closing, expiring, overdue, standing, why, Arranged, CameTo, Expiry};
use crate::app::Ctx;
use crate::config::Settings;
use crate::ports::service::HouseholdRequest;
use crate::test_support::{a_context, a_password, env_at, SeedFs};

/// A moment the calendar holds, for these requests to have been asked at.
const ASKED: &str = "2026-08-17T21:04:09";

/// A context whose settings point at a scratch install of its own.
fn ctx(name: &str) -> crate::app::Ctx {
    a_context()
        .settings(Settings {
            env_file: Some(env_at(name, &a_password())),
            ..Settings::default()
        })
        .build()
}

/// One request as the service records it, at the status that decides its state.
fn asked(id: i64, made: Option<&str>, request_status: u8) -> HouseholdRequest {
    HouseholdRequest {
        id,
        made: made.map(str::to_owned),
        member: "Ana".to_owned(),
        kind: Some(crate::recyclarr::Kind::Radarr),
        item: None,
        request_status,
        media_status: 2,
    }
}

/// The moment these requests were asked at, as an instant.
fn when() -> SystemTime {
    crate::instant::read(ASKED).unwrap_or(UNIX_EPOCH)
}

/// Naming a period records it, and a second run finds it.
#[test]
fn naming_a_period_records_it_for_the_run_that_acts_on_it() {
    let ctx = ctx("named");

    let arranged = agreeing(&ctx, Arranged::After(30)).ok();
    assert_eq!(arranged.and_then(|agreed| agreed.after()), Some(30));

    let found = agreeing(&ctx, Arranged::AsAgreed).ok();
    assert_eq!(found.and_then(|agreed| agreed.after()), Some(30));
}

/// Withdrawing it leaves a household closing nothing again.
#[test]
fn withdrawing_it_leaves_the_household_closing_nothing() {
    let ctx = ctx("withdrawn-here");
    assert!(agreeing(&ctx, Arranged::After(30)).is_ok());

    assert_eq!(
        agreeing(&ctx, Arranged::Never)
            .ok()
            .map(|agreed| agreed.after()),
        Some(None)
    );
    assert_eq!(
        agreeing(&ctx, Arranged::AsAgreed)
            .err()
            .map(|problem| problem.code),
        Some(crate::error::codes::quota::NOTHING_AGREED),
        "a withdrawn arrangement was run on anyway"
    );
}

/// A run asked to begin against no arrangement is refused rather than given one.
#[test]
fn beginning_against_no_arrangement_is_refused() {
    let refused = agreeing(&ctx("never-arranged"), Arranged::AsAgreed).err();

    assert_eq!(
        refused.map(|problem| problem.code),
        Some(crate::error::codes::quota::NOTHING_AGREED)
    );
}

/// A period sooner than the reminder is refused, and records nothing.
#[test]
fn a_period_sooner_than_the_reminder_arranges_nothing() {
    let ctx = ctx("too-soon");

    let refused = agreeing(&ctx, Arranged::After(3)).err();

    assert_eq!(
        refused.map(|problem| problem.code),
        Some(crate::error::codes::quota::TOO_SOON)
    );
    assert_eq!(
        crate::app::arrangement::load(&ctx).after(),
        None,
        "a period this refused was written down anyway"
    );
}

/// A rehearsal says what it would arrange and arranges nothing.
#[test]
fn a_rehearsal_arranges_nothing() {
    let rehearsing = ctx("rehearsed").rehearsing();

    let agreed = agreeing(&rehearsing, Arranged::After(30)).ok();

    assert_eq!(agreed.and_then(|held| held.after()), Some(30));
    assert_eq!(
        crate::app::arrangement::load(&rehearsing).after(),
        None,
        "a rehearsal arranged something"
    );
}

/// A rehearsal of a withdrawal withdraws nothing either.
///
/// The half of a rehearsal easiest to leave out, and the one that would cost most:
/// an operator checking what withdrawing would do, and finding it done.
#[test]
fn a_rehearsed_withdrawal_leaves_the_arrangement_standing() {
    let arranged = ctx("rehearsed-withdrawal");
    assert!(agreeing(&arranged, Arranged::After(30)).is_ok());
    let rehearsing = arranged.rehearsing();

    let said = agreeing(&rehearsing, Arranged::Never).ok();

    assert_eq!(said.map(|held| held.after()), Some(None));
    assert_eq!(
        crate::app::arrangement::load(&rehearsing).after(),
        Some(30),
        "a rehearsal withdrew the arrangement"
    );
}

/// An install with nowhere to record it refuses to arrange anything.
#[test]
fn nowhere_to_record_it_arranges_nothing() {
    let nowhere = a_context().build();

    assert!(agreeing(&nowhere, Arranged::After(30)).is_err());
    assert!(agreeing(&nowhere, Arranged::Never).is_err());
}

/// Only what nobody has ruled on and has waited long enough is closed.
#[test]
fn only_what_nobody_ruled_on_and_waited_long_enough_is_closed() {
    // Two waiting on somebody, and one an operator already approved. The dates are
    // the same on purpose: what tells them apart here is the status, and the period
    // is what the second reading below turns on.
    let held = [
        asked(1, Some(ASKED), 1),
        asked(2, Some(ASKED), 1),
        asked(3, Some(ASKED), 2),
    ];
    let closing = overdue(&held, 30, when() + Duration::from_secs(86_400 * 45));

    assert_eq!(
        closing.iter().map(|held| held.id).collect::<Vec<i64>>(),
        vec![1, 2],
        "the wrong requests were reached"
    );
    assert!(
        overdue(&held, 30, when() + Duration::from_secs(86_400 * 29)).is_empty(),
        "a request inside the period was closed"
    );
}

/// A request this cannot date is never closed.
///
/// It has not been shown to have waited at all, and closing one on no evidence is
/// the opposite of what a period is for.
#[test]
fn a_request_with_no_readable_date_is_never_closed() {
    let held = [asked(1, None, 1), asked(2, Some("tuesday"), 1)];

    assert!(overdue(&held, 30, when() + Duration::from_secs(86_400 * 400)).is_empty());
}

/// The arrangement says what runs it, because nothing does.
#[test]
fn the_arrangement_says_that_nothing_runs_it() {
    let arranged = standing(
        &a_context().build(),
        &Expiry::agreed_to(30, None),
        Arranged::After(30),
    );

    assert!(arranged.contains("30 days"), "{arranged}");
    assert!(arranged.contains("starts nothing by itself"), "{arranged}");
    assert!(
        arranged.contains("lemonfiber household expiring"),
        "{arranged}"
    );
}

/// A rehearsal of the arrangement says it arranged nothing.
#[test]
fn a_rehearsed_arrangement_says_so() {
    let rehearsing = a_context().build().rehearsing();

    let said = standing(
        &rehearsing,
        &Expiry::agreed_to(30, None),
        Arranged::After(30),
    );

    assert!(said.contains("nothing was arranged"), "{said}");
}

/// Withdrawing it is said as the household closing nothing, not as a period of nought.
#[test]
fn withdrawing_it_is_said_as_closing_nothing() {
    let said = standing(&a_context().build(), &Expiry::default(), Arranged::Never);

    assert!(said.contains("waits until somebody rules on it"), "{said}");
    assert!(!said.contains("0 days"), "{said}");
}

/// A run that begins says it holds until it is stopped.
#[test]
fn a_run_that_begins_says_it_holds_until_it_is_stopped() {
    let said = standing(
        &a_context().build(),
        &Expiry::agreed_to(30, None),
        Arranged::AsAgreed,
    );

    assert!(said.contains("left running"), "{said}");
}

/// A run that closed nothing says so rather than saying nothing.
#[test]
fn a_run_that_closed_nothing_says_so() {
    let said = CameTo::default().said(&a_context().build(), 30);

    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said.first()
            .is_some_and(|line| line.contains("nothing was closed")),
        "{said:?}"
    );
}

/// What was closed is counted, and what could not be looked at is said beside it.
#[test]
fn what_was_closed_is_counted_and_what_was_missed_is_said_beside_it() {
    let mut came_to = CameTo {
        closed: 2,
        ..CameTo::default()
    };
    came_to.missed("the request service would not answer");
    came_to.missed("the media server would not answer");

    let said = came_to.said(&a_context().build(), 30);

    assert!(
        said.first()
            .is_some_and(|line| line.contains("closed 2 requests")),
        "{said:?}"
    );
    assert!(
        said.get(1)
            .is_some_and(|line| line.contains("2 looks came to nothing")),
        "{said:?}"
    );
    assert!(
        said.get(1)
            .is_some_and(|line| line.contains("media server")),
        "the last reason was not the one reported: {said:?}"
    );
}

/// A rehearsal counts what it would have closed rather than what it did.
#[test]
fn a_rehearsal_counts_what_it_would_have_closed() {
    let rehearsing = a_context().build().rehearsing();

    let said = CameTo {
        closed: 1,
        ..CameTo::default()
    }
    .said(&rehearsing, 30);

    assert!(
        said.first()
            .is_some_and(|line| line.contains("would close 1 request")),
        "{said:?}"
    );
}

/// What the person who asked is told carries why and never the decline.
#[test]
fn what_travels_is_why_and_never_the_decline_itself() {
    let said = why(30);

    assert!(said.contains("30 days"), "{said}");
    assert!(said.contains("ask again"), "{said}");
    for absent in ["declined", "lemonfiber", "http"] {
        assert!(!said.contains(absent), "{absent} travelled: {said}");
    }
}

/// A Servarr config that opens a target, carrying a readable key.
const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

/// The one request the household is waiting on, asked for long enough ago that any
/// period these cases name has run out on it.
///
/// **Dated years back on purpose.** These cases run on the real clock rather than a
/// stopped one, so a date near today would be inside a thirty-day period this month
/// and outside it the next — a test that passes or fails by the calendar.
const WAITING: &str = r#"{"pageInfo":{"results":1},"results":[{"id":7,
    "createdAt":"2020-01-01T00:00:00.000Z","status":1,"type":"movie",
    "media":{"status":2,"externalServiceId":3},
    "requestedBy":{"displayName":"Alex"}}]}"#;

/// Where the member who asked already hears from the request service.
const REACHED_AT: &str = r#"{"pushoverUserKey":"the-user-key",
    "pushoverApplicationToken":"the-application-token",
    "notificationTypes":{"pushover":64}}"#;

/// A request service that answers everything one sweep asks of it.
///
/// Answered by route rather than in turn, because a sweep reads before it writes and
/// a queue would prove only that the right number of calls went out.
fn service(closes: u16, lists: u16) -> Arc<Transport> {
    Transport::by_rules(vec![
        (
            None,
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        (None, "/auth/jellyfin", Answer::reply(200, "{}")),
        (
            Some(crate::ports::http::Method::Get),
            "/request/7",
            Answer::reply(
                200,
                r#"{"id":7,"requestedBy":{"id":4,"displayName":"Alex"}}"#,
            ),
        ),
        (None, "/request/7/", Answer::reply(closes, "{}")),
        (
            None,
            "/user/4/settings/notifications",
            Answer::reply(200, REACHED_AT),
        ),
        (None, "/api/v1/request", Answer::reply(lists, WAITING)),
        (None, "", Answer::reply(200, "[]")),
    ])
}

/// An install that reaches that service, keeping the transport so what it was sent
/// can be read back.
fn install(name: &str, closes: u16) -> (Ctx, Arc<Transport>) {
    listing(name, closes, 200)
}

/// The same, with the request service's own list answering as given.
fn listing(name: &str, closes: u16, lists: u16) -> (Ctx, Arc<Transport>) {
    let transport = service(closes, lists);
    let held: Arc<Transport> = Arc::clone(&transport);
    let mut ctx = a_context()
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .with_http(held);
    ctx.settings.env_file = Some(env_at(name, &a_password()));
    crate::app::targets::record_secret(
        &ctx,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        &a_password(),
    );
    (ctx, transport)
}

/// Whether the transport was asked to turn request seven down.
fn declined(transport: &Arc<Transport>) -> bool {
    transport
        .requests()
        .iter()
        .any(|asked| asked.url.contains("/request/7/decline"))
}

/// A sweep closes what has waited too long, and carries the reason to whoever asked.
///
/// **The whole of the requirement in one run**: it closed, it told them why, and
/// what it told them is on the record for the reading afterwards.
#[tokio::test]
async fn a_sweep_closes_what_waited_too_long_and_tells_whoever_asked() {
    let (ctx, transport) = install("swept", 200);

    let came_to = closing(&ctx, 30, Duration::ZERO).await;

    assert_eq!(came_to.closed, 1, "nothing was closed");
    assert!(declined(&transport), "the request service was never asked");
    let carried = transport
        .requests()
        .into_iter()
        .find(|asked| asked.url.contains("pushover.net"))
        .and_then(|asked| asked.body)
        .unwrap_or_default();
    assert!(carried.contains("30 days"), "{carried}");
    assert!(carried.contains(r#""title":"Why""#), "{carried}");
    assert!(
        !carried.contains("declined"),
        "the decline the service already sent travelled again: {carried}"
    );
    assert_eq!(
        crate::app::refusals::load(&ctx)
            .of(7)
            .map(|kept| kept.expired),
        Some(true),
        "what ran out was not recorded as having run out"
    );
}

/// A run holding the household to a period nobody agrees to any more ends.
///
/// Both ways it can stop being the arrangement in force: withdrawn, and replaced.
/// A run that went on would be applying one nobody currently agrees to, which is
/// exactly the silent policy the whole arrangement exists not to be.
#[tokio::test]
async fn a_run_ends_when_the_arrangement_it_began_under_does() {
    let (withdrawn, _) = install("gone", 200);
    assert_eq!(closing(&withdrawn, 30, Duration::ZERO).await.closed, 1);

    let (replaced, transport) = install("replaced", 200);
    assert!(agreeing(&replaced, Arranged::After(45)).is_ok());

    assert_eq!(closing(&replaced, 30, Duration::ZERO).await.closed, 1);
    assert!(declined(&transport), "the sweep it did make was skipped");
}

/// A request service that will not close one says so, and leaves it waiting.
#[tokio::test]
async fn a_request_the_service_will_not_close_is_left_waiting_and_said() {
    let (ctx, transport) = install("refused-close", 500);

    let came_to = closing(&ctx, 30, Duration::ZERO).await;

    assert_eq!(came_to.closed, 0);
    assert!(
        came_to
            .why
            .is_some_and(|why| why.contains("still waiting on you")),
        "the failure was swallowed"
    );
    assert!(
        !transport
            .requests()
            .iter()
            .any(|asked| asked.url.contains("pushover.net")),
        "somebody was told about a closure that did not happen"
    );
}

/// A household nothing can be read from is a look that came to nothing, not a stop.
///
/// A service down for an hour is one to ask again next hour. Ending a month-long run
/// over a restart would be a clock that stopped the first time anything moved.
#[tokio::test]
async fn a_household_that_cannot_be_read_is_a_look_that_came_to_nothing() {
    let came_to = closing(&a_context().build(), 30, Duration::ZERO).await;

    assert_eq!(came_to.closed, 0);
    assert_eq!(came_to.missed, 1);
    assert!(came_to.why.is_some(), "nothing said why the look failed");
}

/// A stack that will not read is a look that came to nothing, and says which.
///
/// Each of the three ways a look can fail is reported in its own words, because the
/// thing an operator would go and fix is different for each: a stack nothing can
/// read, a request service that will not let this in, and one that will not say what
/// it has been asked for.
#[tokio::test]
async fn each_way_a_look_can_fail_is_said_in_its_own_words() {
    let unreadable = closing(
        &a_context().over(crate::test_support::nowhere()).build(),
        30,
        Duration::ZERO,
    )
    .await;
    assert_eq!(unreadable.missed, 1);

    let (silent, transport) = listing("unlisted", 200, 500);
    let unlisted = closing(&silent, 30, Duration::ZERO).await;

    assert_eq!(unlisted.closed, 0);
    assert!(
        unlisted
            .why
            .is_some_and(|why| why.contains("could not be read")),
        "a service that would not list was not said to have been asked"
    );
    assert!(
        !declined(&transport),
        "something was closed off a list nobody read"
    );
}

/// Asked for as a command, arranging a period answers with the household under it.
#[tokio::test]
async fn arranging_a_period_as_a_command_answers_with_the_household() {
    let (ctx, _) = install("as-a-command", 200);

    let report = crate::app::dispatch(crate::app::Command::Expiring(Arranged::After(30)), &ctx)
        .await
        .ok();

    assert!(
        matches!(report, Some(crate::app::Outcome::Household(ref held))
            if held.findings.iter().any(|line| line.contains("30 days"))),
        "{report:?}"
    );
    assert_eq!(crate::app::arrangement::load(&ctx).after(), Some(30));
}

/// Asked to begin against nothing, as a command, it refuses rather than choosing.
#[tokio::test]
async fn beginning_against_nothing_is_refused_as_a_command_too() {
    let (ctx, _) = install("unarranged", 200);

    let refused = crate::app::dispatch(crate::app::Command::Expiring(Arranged::AsAgreed), &ctx)
        .await
        .err();

    assert_eq!(
        refused.map(|problem| problem.code),
        Some(crate::error::codes::quota::NOTHING_AGREED)
    );
}

/// A rehearsal of a run names what it would close and closes nothing.
#[tokio::test]
async fn a_rehearsed_run_closes_nothing() {
    let (arranged, transport) = install("rehearsed-run", 200);
    assert!(agreeing(&arranged, Arranged::After(30)).is_ok());
    let ctx = arranged.rehearsing();

    let report = expiring(&ctx, Arranged::AsAgreed, Duration::ZERO)
        .await
        .ok();

    assert!(
        report.is_some_and(|held| held
            .findings
            .iter()
            .any(|line| line.contains("would close 1 request"))),
        "a rehearsal did not say what it would close"
    );
    assert!(!declined(&transport), "a rehearsal closed something");
}
