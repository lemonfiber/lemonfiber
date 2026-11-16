//! Signing a household member in by a code the outside service holding their account
//! shows them: the code asked for, the sign-in finished once they approve it, for the
//! session or device that asked alone, and the codes one source may ask for limited.

use lemonfiber_api::admission::coded::CODE;
use lemonfiber_core::error::codes::admit::{
    CODE_NOT_WAITING, NOT_A_CODE_REQUEST, SIGNS_IN_BY_PASSWORD, TOO_MANY_CODES,
};
use lemonfiber_fixtures::household::{APPROVE_AT, CODE as SHOWN, CODE_ID};
use tower::ServiceExt as _;

use crate::door;
use door::*;

/// The id the household files the member under.
const MEMBER: &str = "a7f3";

/// A code asked for, or a sign-in finished, at `router` from `from`, carrying `session`
/// where one is given.
async fn at_the_door(
    router: axum::Router,
    from: IpAddr,
    session: Option<&str>,
    body: &str,
) -> Answer {
    let mut building = Request::builder()
        .method("POST")
        .uri(CODE)
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in from_here() {
        building = building.header(name, value);
    }
    if let Some(session) = session {
        building = building.header(TOKEN_HEADER, session);
    }
    let Ok(mut request) = building.body(Body::from(body.to_owned())) else {
        unreachable!("the request a test writes is one that can be built")
    };
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::new(
            from, 50_000,
        )));
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

/// The value at `pointer` in an answer's body, where it holds a string there.
fn said(body: &str, pointer: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .pointer(pointer)?
        .as_str()
        .map(str::to_owned)
}

/// A surface over a household signing `MEMBER` in by a code, the operator's password kept.
fn signing_in_by_code(named: &str) -> (axum::Router, Arc<Recognising>) {
    let household = Recognising::by_code(MEMBER);
    let (router, _, _) = door_with(
        Some(keeping(named)),
        Arc::clone(&household),
        Chance::turning(1),
    );
    (router, household)
}

/// A session the operator opened at `router` from `from`.
async fn an_operator_session(router: axum::Router, from: IpAddr) -> String {
    session(&offered_from(router, from, &offering(&chosen())).await.body)
}

/// The handle a code asked for at `router` was answered with.
async fn a_handle(router: axum::Router, from: IpAddr, session: Option<&str>) -> String {
    said(
        &at_the_door(router, from, session, "{}").await.body,
        "/data/waiting",
    )
    .unwrap_or_default()
}

/// A body finishing the sign-in `handle` names.
fn finishing(handle: &str) -> String {
    format!("{{\"waiting\":{}}}", serde_json::json!(handle))
}

#[tokio::test]
async fn a_code_is_answered_with_where_to_approve_it_and_a_handle_never_the_sources_id() {
    let (router, _) = signing_in_by_code("code-asked");

    let answer = at_the_door(router, a_device(7), None, "{}").await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(said(&answer.body, "/kind").as_deref(), Some("sign-in-code"));
    assert_eq!(said(&answer.body, "/data/code").as_deref(), Some(SHOWN));
    assert_eq!(
        said(&answer.body, "/data/approve_at").as_deref(),
        Some(APPROVE_AT)
    );
    assert_eq!(
        said(&answer.body, "/data/waiting").map(|handle| handle.len()),
        Some(32)
    );
    assert!(!answer.body.contains(CODE_ID), "{}", answer.body);
}

#[tokio::test]
async fn a_sign_in_waits_until_approved_then_opens_the_members_session_once() {
    let (router, household) = signing_in_by_code("code-finished");
    let handle = a_handle(router.clone(), a_device(7), None).await;

    let waiting = at_the_door(router.clone(), a_device(7), None, &finishing(&handle)).await;
    household.approve();
    let finished = at_the_door(router.clone(), a_device(8), None, &finishing(&handle)).await;
    let again = at_the_door(router, a_device(7), None, &finishing(&handle)).await;

    assert_eq!(waiting.status, StatusCode::OK);
    assert_eq!(
        said(&waiting.body, "/kind").as_deref(),
        Some("sign-in-code")
    );
    assert_eq!(said(&waiting.body, "/data/code"), None);
    assert_eq!(said(&waiting.body, "/data/waiting"), Some(handle));
    assert_eq!(finished.status, StatusCode::OK, "{}", finished.body);
    assert_eq!(said(&finished.body, "/kind").as_deref(), Some("admission"));
    assert_eq!(whose(&finished.body).as_deref(), Some(MEMBER));
    assert!(!session(&finished.body).is_empty());
    assert_eq!(again.status, StatusCode::UNAUTHORIZED);
    assert!(again.body.contains(CODE_NOT_WAITING.as_str()));
}

#[tokio::test]
async fn a_second_session_cannot_finish_the_sign_in_the_first_asked_for() {
    let (router, household) = signing_in_by_code("code-bound");
    let first = an_operator_session(router.clone(), a_device(7)).await;
    let second = an_operator_session(router.clone(), a_device(7)).await;
    assert_ne!(first, second);
    let handle = a_handle(router.clone(), a_device(7), Some(&first)).await;
    household.approve();

    let theirs = at_the_door(
        router.clone(),
        a_device(7),
        Some(&second),
        &finishing(&handle),
    )
    .await;
    let nobodys = at_the_door(router.clone(), a_device(7), None, &finishing(&handle)).await;
    let its_own = at_the_door(router, a_device(7), Some(&first), &finishing(&handle)).await;

    for refused in [&theirs, &nobodys] {
        assert_eq!(refused.status, StatusCode::UNAUTHORIZED);
        assert!(refused.body.contains(CODE_NOT_WAITING.as_str()));
    }
    assert_eq!(its_own.status, StatusCode::OK, "{}", its_own.body);
    assert_eq!(whose(&its_own.body).as_deref(), Some(MEMBER));
}

#[tokio::test]
async fn one_source_flooding_meets_its_own_limit_while_another_is_still_given_codes() {
    let (router, _) = signing_in_by_code("code-flooded");
    for _ in 0..3 {
        assert!(!a_handle(router.clone(), a_device(7), None).await.is_empty());
    }

    let flooded = at_the_door(router.clone(), a_device(7), None, "{}").await;
    let other = at_the_door(router, a_device(8), None, "{}").await;

    assert_eq!(flooded.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(flooded.body.contains(TOO_MANY_CODES.as_str()));
    assert_eq!(flooded.left.as_deref(), Some("600"));
    assert_eq!(other.status, StatusCode::OK, "{}", other.body);
}

#[tokio::test]
async fn a_household_whose_members_sign_in_by_a_password_is_asked_for_no_code() {
    let (router, _, _) = door_with(
        Some(keeping("code-by-password")),
        Recognising::knowing(MEMBER),
        Chance::turning(1),
    );

    let answer = at_the_door(router, a_device(7), None, "{}").await;

    assert_eq!(answer.status, StatusCode::CONFLICT);
    assert!(answer.body.contains(SIGNS_IN_BY_PASSWORD.as_str()));
}

#[tokio::test]
async fn a_body_that_neither_asks_nor_finishes_is_refused_and_an_unknown_handle_finishes_nothing() {
    let (router, _) = signing_in_by_code("code-unread");

    let unread = at_the_door(router.clone(), a_device(7), None, r#"{"code":"WXYZ"}"#).await;
    let unknown = at_the_door(router, a_device(7), None, &finishing("0f0f")).await;

    assert_eq!(unread.status, StatusCode::BAD_REQUEST);
    assert!(unread.body.contains(NOT_A_CODE_REQUEST.as_str()));
    assert_eq!(unknown.status, StatusCode::UNAUTHORIZED);
    assert!(unknown.body.contains(CODE_NOT_WAITING.as_str()));
}

#[tokio::test]
async fn a_household_that_cannot_be_asked_issues_no_code_and_finishes_nothing() {
    let (router, household) = signing_in_by_code("code-dark");
    let handle = a_handle(router.clone(), a_device(7), None).await;
    household.go_dark();

    let asked = at_the_door(router.clone(), a_device(8), None, "{}").await;
    let finished = at_the_door(router, a_device(7), None, &finishing(&handle)).await;

    for answer in [asked, finished] {
        assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    }
}

#[tokio::test]
async fn the_device_a_code_is_asked_for_is_drawn_by_the_core_and_never_named_by_the_caller() {
    let (router, household) = signing_in_by_code("code-device");

    let named = at_the_door(router.clone(), a_device(7), None, r#"{"device":"tv-1"}"#).await;
    let first = at_the_door(router.clone(), a_device(7), None, "{}").await;
    let second = at_the_door(router, a_device(7), None, "{}").await;
    let devices = household.devices();

    assert_eq!(named.status, StatusCode::BAD_REQUEST);
    assert!(named.body.contains(NOT_A_CODE_REQUEST.as_str()));
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(second.status, StatusCode::OK);
    assert_eq!(devices.len(), 2, "{devices:?}");
    assert_ne!(devices.first(), devices.get(1));
    assert!(devices
        .iter()
        .all(|device| device.len() == 32 && device.bytes().all(|byte| byte.is_ascii_hexdigit())));
}

#[tokio::test]
async fn no_code_is_issued_where_no_device_or_no_handle_can_be_drawn() {
    for random in [Chance::exactly(None), Chance::exactly(Some(vec![1; 32]))] {
        let (router, _, _) = door_with(
            Some(keeping("code-undrawn")),
            Recognising::by_code(MEMBER),
            random,
        );

        let answer = at_the_door(router, a_device(7), None, "{}").await;

        assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    }
}

/// A household that can be made to vanish, or to stop vouching for anybody, after a code
/// was asked of it.
struct Fading {
    household: Arc<Recognising>,
    gone: AtomicBool,
    vouching: AtomicBool,
}

#[async_trait::async_trait]
impl HouseholdAtHand for Fading {
    async fn now(&self) -> Option<Arc<dyn Household>> {
        (!self.gone.load(Ordering::SeqCst))
            .then(|| Arc::clone(&self.household) as Arc<dyn Household>)
    }

    fn vouches_for(&self, _: &str) -> bool {
        self.vouching.load(Ordering::SeqCst)
    }
}

/// A surface over a household that fades as `fade` says once a code was asked of it, and
/// the answer finishing that code.
async fn finished_after(named: &str, fade: impl Fn(&Fading)) -> Answer {
    let household = Recognising::by_code(MEMBER);
    household.approve();
    let fading = Arc::new(Fading {
        household,
        gone: AtomicBool::new(false),
        vouching: AtomicBool::new(true),
    });
    let path = Some(keeping(named));
    let admitting = Arc::new(Admitting {
        kept: path.clone(),
        household: Some(Arc::clone(&fading) as Arc<dyn HouseholdAtHand>),
        ..Admitting::default()
    });
    let (router, _) = surface(world(path, Chance::turning(1)), &admitting);
    let handle = a_handle(router.clone(), a_device(7), None).await;
    fade(&fading);
    at_the_door(router, a_device(7), None, &finishing(&handle)).await
}

#[tokio::test]
async fn a_sign_in_finishes_nothing_where_the_household_is_gone_withdrew_the_member_or_vouches_for_nobody(
) {
    let gone = finished_after("code-gone", |fading| {
        fading.gone.store(true, Ordering::SeqCst);
    })
    .await;
    let withdrawn = finished_after("code-withdrawn", |fading| fading.household.withdraw()).await;
    let unvouched = finished_after("code-unvouched", |fading| {
        fading.vouching.store(false, Ordering::SeqCst);
    })
    .await;

    for refused in [&gone, &withdrawn] {
        assert_eq!(refused.status, StatusCode::FORBIDDEN, "{}", refused.body);
    }
    assert_eq!(
        unvouched.status,
        StatusCode::UNAUTHORIZED,
        "{}",
        unvouched.body
    );
    assert!(unvouched.body.contains(CODE_NOT_WAITING.as_str()));
}

#[tokio::test]
async fn two_finishes_of_one_approved_code_at_once_open_exactly_one_session() {
    let (router, household) = signing_in_by_code("code-raced");
    let handle = a_handle(router.clone(), a_device(7), None).await;
    household.approve();
    let body = finishing(&handle);

    let (one, other) = tokio::join!(
        at_the_door(router.clone(), a_device(7), None, &body),
        at_the_door(router, a_device(7), None, &body),
    );
    let admitted = [&one, &other]
        .iter()
        .filter(|answer| said(&answer.body, "/kind").as_deref() == Some("admission"))
        .count();
    let refused = [&one, &other]
        .iter()
        .filter(|answer| {
            answer.status == StatusCode::UNAUTHORIZED
                && answer.body.contains(CODE_NOT_WAITING.as_str())
        })
        .count();

    assert_eq!((admitted, refused), (1, 1), "{}\n{}", one.body, other.body);
}

#[tokio::test]
async fn a_stack_with_no_household_to_ask_issues_no_code() {
    let (router, _, _) = door(Some(keeping("code-no-household")), Chance::turning(1));

    let answer = at_the_door(router, a_device(7), None, "{}").await;

    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
}
