//! The one door that opens without a token, and what it counts against guessing.
//!
//! Driven from outside the crate, because what a caller can reach is the thing worth
//! holding still — and because everything here is asynchronous, which is a shape the
//! coverage gate reads properly only from out here.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use lemonfiber_api::admission::{Admitting, RETRY_AFTER, SESSION};
use lemonfiber_api::events::live::Live;
use lemonfiber_api::events::Streaming;
use lemonfiber_api::guard::{Binding, Token, TOKEN_HEADER};
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::router::{routes, Serving};
use lemonfiber_core::admission::{credential, Credential};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::ports::{Chance, Idle, Stopped};
use lemonfiber_fixtures::support::{a_password, Reporting};
use tower::ServiceExt as _;

/// The second the stopped clock reads.
const NOW: u64 = 1_700_000_000;

/// The port this surface says it is listening on.
const PORT: u16 = 8471;

/// Serving this machine and nowhere else.
fn bound() -> Binding {
    Binding::here(PORT)
}

/// What a request has to say to have come from here.
fn from_here() -> Vec<(&'static str, String)> {
    vec![("host", format!("127.0.0.1:{PORT}"))]
}

/// The moment every one of these runs at.
fn moment() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(NOW)
}

/// A directory of this test's own, emptied first so a rerun starts fresh.
fn a_directory(named: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-door-{named}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// The password the operator chose, built rather than written down.
fn chosen() -> String {
    a_password()
}

/// The credential that password makes.
fn a_credential(password: &str) -> Credential {
    let Ok(held) = Credential::set(password, &Chance::cycling()) else {
        unreachable!("a full-length password and a source that answers make a record")
    };
    held
}

/// A machine keeping that password, and where it keeps it.
fn keeping(named: &str) -> PathBuf {
    let path = a_directory(named).join("admission.json");
    let held = a_credential(&chosen());
    let Ok(()) = credential::keep(&path, &held) else {
        unreachable!("a scratch directory can be written")
    };
    path
}

/// The world these requests run against: a stopped clock, and randomness a test chose.
fn world(admission: Option<PathBuf>, random: Chance) -> Ctx {
    Ctx::new(
        Arc::new(Idle),
        Arc::new(Reporting::absent()),
        Stopped::at(NOW),
        Arc::new(lemonfiber_core::adapters::Disk),
        Source::External(Path::new("/lemonfiber/no/such/stack")),
        Settings {
            admission,
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(Fake::silent())
    .with_random(Arc::new(random))
}

/// The whole surface over that world, and the register it admits from.
fn surface(ctx: Ctx, admitting: &Arc<Admitting>) -> (axum::Router, Arc<Token>) {
    let Some(token) = Token::mint(&Chance::cycling()).map(Arc::new) else {
        unreachable!("a cycling source always mints one")
    };
    let live = Arc::new(Live::opening(Stopped::at(0).as_ref()));
    let serving = Serving {
        ctx: Arc::new(ctx),
        token: Arc::clone(&token),
        bound: bound(),
        jobs: Jobs::default(),
        admitting: Arc::clone(admitting),
        live: Arc::clone(&live),
    };
    let streaming = Arc::new(Streaming {
        token: Arc::clone(&token),
        bound: bound(),
        admitting: Arc::clone(admitting),
        live,
    });
    (routes(serving, streaming), token)
}

/// A surface keeping the password at `path`, sharing one register with the test.
fn door(path: Option<PathBuf>, random: Chance) -> (axum::Router, Arc<Token>, Arc<Admitting>) {
    let admitting = Arc::new(Admitting {
        kept: path.clone(),
        ..Admitting::default()
    });
    let (router, token) = surface(world(path, random), &admitting);
    (router, token, admitting)
}

/// What one request was answered with: the status, the body, and how long is left.
struct Answer {
    /// The status it came back under.
    status: StatusCode,
    /// What it said.
    body: String,
    /// The wait it named, where it named one.
    left: Option<String>,
}

/// One request, and what it was answered with.
async fn asked(
    router: axum::Router,
    method: &str,
    path: &str,
    pairs: &[(&str, String)],
    body: &str,
) -> Answer {
    let mut building = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in pairs {
        building = building.header(*name, value);
    }
    let Ok(request) = building.body(Body::from(body.to_owned())) else {
        unreachable!("the request a test writes is one that can be built")
    };
    let Ok(response) = router.oneshot(request).await;
    let status = response.status();
    let left = response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let Ok(read) = to_bytes(response.into_body(), 64 * 1024).await else {
        unreachable!("an answer this surface produces is one that can be read")
    };
    Answer {
        status,
        body: String::from_utf8_lossy(&read).into_owned(),
        left,
    }
}

/// The body a password is offered in.
fn offering(password: &str) -> String {
    format!("{{\"password\":{}}}", serde_json::json!(password))
}

/// The secret an answer handed back, where it handed one back.
fn session(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .as_ref()
        .and_then(|opened| opened.pointer("/data/token"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test]
async fn the_right_password_is_exchanged_for_a_session_the_rest_of_the_surface_takes() {
    let path = keeping("exchanged");
    let (router, _, admitting) = door(Some(path.clone()), Chance::cycling());

    let answer = asked(router, "POST", SESSION, &from_here(), &offering(&chosen())).await;

    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        answer
            .body
            .starts_with(r#"{"api_version":1,"kind":"admission","data":{"token":"#),
        "{}",
        answer.body
    );
    let opened = session(&answer.body);
    assert!(!opened.is_empty(), "{}", answer.body);
    // And what it handed back is a secret the rest of the surface takes, which is the
    // whole of what being given one is worth.
    assert!(
        admitting
            .sessions
            .holds(Some(&opened), moment(), &a_credential(&chosen()))
            .await
    );
    let _ = fs::remove_dir_all(a_directory("exchanged"));
}

#[tokio::test]
async fn a_wrong_password_and_a_machine_with_none_are_refused_the_same_way() {
    // The same sentence and the same status for both, because they are the same fact
    // to whoever is knocking: what they sent did not open the door. Saying which of
    // the two it was would tell somebody guessing whether there is anything to guess.
    let path = keeping("wrong");
    let (router, _, _) = door(Some(path), Chance::cycling());
    let wrong = asked(
        router,
        "POST",
        SESSION,
        &from_here(),
        &offering(&chosen().to_uppercase()),
    )
    .await;

    let (bare, _, _) = door(None, Chance::cycling());
    let unset = asked(bare, "POST", SESSION, &from_here(), &offering(&chosen())).await;

    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    assert_eq!(unset.status, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong.body, unset.body);
    let _ = fs::remove_dir_all(a_directory("wrong"));
}

#[tokio::test]
async fn a_body_that_is_not_a_password_is_said_plainly_rather_than_counted() {
    let (router, _, admitting) = door(None, Chance::cycling());

    let answer = asked(router, "POST", SESSION, &from_here(), r#"{"pass":1}"#).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert!(answer.body.contains("not a password"), "{}", answer.body);
    // A request that was never an answer is not a wrong answer, so nothing is owed.
    assert_eq!(admitting.attempts.waiting(moment()).await, None);
}

#[tokio::test]
async fn wrong_answers_are_free_for_a_while_and_then_made_to_wait() {
    let path = keeping("counted");
    let (_, _, admitting) = door(Some(path.clone()), Chance::cycling());

    // Three cost nothing, which is a mistyped password, a forgotten capital, and one
    // more.
    for _ in 0..3u8 {
        admitting.attempts.wrong(moment()).await;
    }
    assert_eq!(admitting.attempts.waiting(moment()).await, None);

    admitting.attempts.wrong(moment()).await;
    let owed = admitting.attempts.waiting(moment()).await;
    assert!(owed.is_some_and(|left| left > Duration::ZERO), "{owed:?}");

    // And the wait ends, rather than the door staying shut.
    assert_eq!(
        admitting
            .attempts
            .waiting(moment() + Duration::from_secs(600))
            .await,
        None
    );

    // A right answer forgets them, so the next mistake starts from nothing again.
    admitting.attempts.wrong(moment()).await;
    admitting.attempts.right().await;
    assert_eq!(admitting.attempts.waiting(moment()).await, None);
    let _ = fs::remove_dir_all(a_directory("counted"));
}

#[tokio::test]
async fn somebody_made_to_wait_is_told_how_long_and_is_told_it_twice() {
    let path = keeping("waiting");
    let (router, _, admitting) = door(Some(path.clone()), Chance::cycling());
    for _ in 0..6u8 {
        admitting.attempts.wrong(moment()).await;
    }

    // The right password, and it is not even looked at: the wait is what is answered.
    let answer = asked(router, "POST", SESSION, &from_here(), &offering(&chosen())).await;

    assert_eq!(answer.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(
        answer.body.contains("Too many wrong passwords"),
        "{}",
        answer.body
    );
    // In the header a client reads and in the sentence a person reads, because both
    // of them are here: the page shows one and the client behind it waits on the other.
    assert!(answer.left.is_some_and(|left| left.parse::<u64>().is_ok()));
    let _ = fs::remove_dir_all(a_directory("waiting"));
}

#[tokio::test]
async fn a_request_that_says_it_came_from_elsewhere_never_reaches_the_door() {
    // The token half of the guard is what this route exists without. This half is not
    // negotiable, and it is what stops a page the operator happens to be visiting from
    // posting guesses here with their browser.
    let path = keeping("elsewhere");
    let (router, _, _) = door(Some(path), Chance::cycling());

    let answer = asked(
        router,
        "POST",
        SESSION,
        &[("host", "example.com:8471".to_owned())],
        &offering(&chosen()),
    )
    .await;

    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    let _ = fs::remove_dir_all(a_directory("elsewhere"));
}

#[tokio::test]
async fn a_run_that_will_not_supply_a_secret_hands_out_no_session() {
    // A session whose secret could not be minted would be one every request reached,
    // so there is nothing here to fall back to.
    let path = keeping("secretless");
    let (router, _, _) = door(Some(path), Chance::exactly(None));

    let answer = asked(router, "POST", SESSION, &from_here(), &offering(&chosen())).await;

    assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR);
    let _ = fs::remove_dir_all(a_directory("secretless"));
}

#[tokio::test]
async fn a_session_ends_when_it_expires_and_when_the_password_changes() {
    let path = keeping("ending");
    let (_, _, admitting) = door(Some(path.clone()), Chance::cycling());
    let held = a_credential(&chosen());
    let opened = admitting
        .sessions
        .opened(&Chance::cycling(), moment(), &held)
        .await;
    let Some(opened) = opened else {
        unreachable!("a cycling source mints a session")
    };

    assert!(
        admitting
            .sessions
            .holds(Some(&opened.token), moment(), &held)
            .await
    );
    // A day later it is gone, because a window left open all week is not evidence
    // that whoever opened it is still there.
    assert!(
        !admitting
            .sessions
            .holds(
                Some(&opened.token),
                moment() + Duration::from_secs(24 * 60 * 60),
                &held
            )
            .await
    );
    let _ = fs::remove_dir_all(a_directory("ending"));
}

#[tokio::test]
async fn opening_one_lets_go_of_the_ones_that_have_ended_and_keeps_the_rest() {
    // Two people are in the house, so a second session does not take the first one
    // away. What opening one does take away is whatever has already ended, which is
    // the only sweep there is — nothing here runs on a timer.
    let path = keeping("several");
    let (_, _, admitting) = door(Some(path.clone()), Chance::cycling());
    let held = a_credential(&chosen());
    let later = moment() + Duration::from_secs(24 * 60 * 60);

    let first = admitting
        .sessions
        .opened(&Chance::cycling(), moment(), &held)
        .await;
    let second = admitting
        .sessions
        .opened(&Chance::exactly(Some(vec![0x7b; 32])), moment(), &held)
        .await;
    let Some((first, second)) = first.zip(second) else {
        unreachable!("both sources mint a session")
    };
    assert_ne!(first.token, second.token);
    assert!(
        admitting
            .sessions
            .holds(Some(&first.token), moment(), &held)
            .await
    );

    // A day on, both have ended, and opening a third is what lets go of them.
    let third = admitting
        .sessions
        .opened(&Chance::exactly(Some(vec![0x2c; 32])), later, &held)
        .await;
    let Some(third) = third else {
        unreachable!("a source that answers mints a session")
    };
    assert!(
        !admitting
            .sessions
            .holds(Some(&first.token), later, &held)
            .await
    );
    assert!(
        admitting
            .sessions
            .holds(Some(&third.token), later, &held)
            .await
    );
    let _ = fs::remove_dir_all(a_directory("several"));
}

#[tokio::test]
async fn a_password_change_ends_a_session_somebody_else_is_holding() {
    let path = keeping("changed");
    let (_, _, admitting) = door(Some(path.clone()), Chance::cycling());
    let held = a_credential(&chosen());
    let opened = admitting
        .sessions
        .opened(&Chance::cycling(), moment(), &held)
        .await;
    let Some(opened) = opened else {
        unreachable!("a cycling source mints a session")
    };

    // Set another password, which is what an operator does when they suspect somebody
    // else is in. The session was opened against the one that is no longer there.
    let another = a_credential(&chosen().to_uppercase());
    assert!(credential::keep(&path, &another).is_ok());

    assert!(
        !admitting
            .sessions
            .holds(Some(&opened.token), moment(), &another)
            .await
    );
    let _ = fs::remove_dir_all(a_directory("changed"));
}

#[tokio::test]
async fn a_secret_this_run_never_handed_out_is_no_session_and_nor_is_nothing() {
    let path = keeping("unknown");
    let (_, _, admitting) = door(Some(path.clone()), Chance::cycling());
    let held = a_credential(&chosen());

    assert!(!admitting.sessions.holds(None, moment(), &held).await);
    assert!(
        !admitting
            .sessions
            .holds(Some("not a session"), moment(), &held)
            .await
    );
    let _ = fs::remove_dir_all(a_directory("unknown"));
}

#[tokio::test]
async fn the_token_still_opens_everything_and_nothing_opens_it_where_no_password_is_kept() {
    let (router, token, admitting) = door(None, Chance::cycling());

    // Nothing is kept, so there is no credential to read and no session to be held.
    assert!(admitting.credential().is_none());
    let mut carried = from_here();
    carried.push((TOKEN_HEADER, token.as_str().to_owned()));
    let answer = asked(router, "GET", "/api/explain?word=indexer", &carried, "").await;

    assert_eq!(answer.status, StatusCode::OK);
}

#[tokio::test]
async fn a_read_carrying_a_session_is_answered_the_same_as_one_carrying_the_token() {
    let path = keeping("reading");
    let (router, _, _) = door(Some(path.clone()), Chance::cycling());
    let opened = asked(
        router.clone(),
        "POST",
        SESSION,
        &from_here(),
        &offering(&chosen()),
    )
    .await;
    let opened = session(&opened.body);
    assert!(!opened.is_empty(), "no session was handed out");

    let mut carried = from_here();
    carried.push((TOKEN_HEADER, opened));
    let answer = asked(router, "GET", "/api/explain?word=indexer", &carried, "").await;

    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.body.contains(r#""kind":"word""#), "{}", answer.body);
    let _ = fs::remove_dir_all(a_directory("reading"));
}

#[tokio::test]
async fn everything_else_still_needs_a_secret_this_run_admits() {
    let path = keeping("guarded");
    let (router, _, _) = door(Some(path.clone()), Chance::cycling());

    let answer = asked(router, "GET", "/api/explain?word=indexer", &from_here(), "").await;

    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    let _ = fs::remove_dir_all(a_directory("guarded"));
}

/// Exactly one path on this surface is let through the token half of the guard.
///
/// Read from the guard's own source rather than by trying every path, because the
/// failure worth catching is a second exemption written beside the first — and a
/// sweep over the paths that exist today would not see one added tomorrow.
#[test]
fn exactly_one_path_is_let_through_without_a_token() {
    let Ok(guard) = fs::read_to_string("src/router.rs") else {
        unreachable!("the guard this crate ships is in the tree this test is built from")
    };
    let exempt: Vec<&str> = guard
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .filter(|line| line.contains("request.uri().path() =="))
        .collect();
    assert_eq!(
        exempt.len(),
        1,
        "the guard lets more than one path through without a token: {exempt:?}"
    );
    assert!(
        exempt
            .first()
            .is_some_and(|line| line.contains("crate::admission::SESSION")),
        "the one path let through is not the door: {exempt:?}"
    );
}
