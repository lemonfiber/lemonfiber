//! What a key a program holds is let in to, and everything it is refused.
//!
//! Every refusal a key can meet is staged here against the whole surface, as a client
//! reaches it: a key from another machine in the clear, a revoked or unknown key, a
//! key asking past its scope, a key asking about keys, and a key arriving while
//! guessing has earned a wait.

use std::net::IpAddr;

use lemonfiber_api::admission::Keyring;
use lemonfiber_api::guard::Arrived;
use lemonfiber_core::keys::{Kept, Minter, Purpose, Record, Scope, Secret};

use crate::door::*;
use tower::ServiceExt as _;

/// This machine, as a connection names it.
pub(crate) const HERE: IpAddr = IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);

/// Another machine on the household network.
pub(crate) const NEXT_DOOR: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 23));

/// A secret minted from a source answering with `byte`.
pub(crate) fn a_secret(byte: u8) -> Secret {
    let Some(secret) = Secret::mint(&Chance::exactly(Some(vec![byte; 32]))) else {
        unreachable!("thirty-two bytes mint a secret")
    };
    secret
}

/// The keys one machine keeps, each with the secret a program holds for it.
struct Keys {
    /// Where they are kept.
    kept: PathBuf,
    /// Where their use is written.
    used: PathBuf,
    /// A `read` key's secret.
    read: Secret,
    /// An `act` key's secret.
    act: Secret,
    /// A revoked key's secret.
    revoked: Secret,
    /// A key scoped to the member the test household holds.
    member: Secret,
}

/// A machine keeping one key of each kind, and its password.
fn keeping_keys(named: &str) -> Keys {
    let dir = a_directory(&format!("keys-{named}"));
    let _ = fs::create_dir_all(&dir);
    let record = |name: &str, scope: Scope, secret: &Secret| {
        Record::minted(
            name,
            scope,
            Purpose::Other,
            secret,
            "2026-10-05T06:00:00".to_owned(),
            Minter::Operator,
        )
    };
    let keys = Keys {
        kept: dir.join("keys.json"),
        used: dir.join("keys-used.json"),
        read: a_secret(1),
        act: a_secret(2),
        revoked: a_secret(3),
        member: a_secret(4),
    };
    let mut gone = record("gone", Scope::Read, &keys.revoked);
    gone.revoked = Some("2026-10-05T07:00:00".to_owned());
    let kept = Kept {
        keys: vec![
            record("reader", Scope::Read, &keys.read),
            record("actor", Scope::Act, &keys.act),
            gone,
            record(
                "anas",
                Scope::Member {
                    id: "a7f3".to_owned(),
                    name: "ana".to_owned(),
                },
                &keys.member,
            ),
        ],
    };
    assert!(kept.keep(&keys.kept).is_ok());
    keys
}

/// A surface admitting those keys, its household holding `household`.
fn surface_with(keys: &Keys, household: Arc<AHousehold>) -> (axum::Router, Arc<Admitting>) {
    let admitting = Arc::new(Admitting {
        keys: Keyring::at(Some(keys.kept.clone()), Some(keys.used.clone())),
        household: Some(Arc::new(household as Arc<dyn Household>) as Arc<dyn HouseholdAtHand>),
        ..Admitting::default()
    });
    let (router, _) = surface(world(None, Chance::cycling()), &admitting);
    (router, admitting)
}

/// One request carrying `secret`, from `from` over a connection encrypted or not, or
/// over one nothing vouched for where `from` is nothing.
pub(crate) async fn presented(
    router: axum::Router,
    method: &str,
    path: &str,
    secret: &str,
    from: Option<(IpAddr, bool)>,
) -> Answer {
    sent(router, method, path, secret, from, "{}").await.0
}

/// One request carrying `secret` and `body`, and the answer with its headers.
pub(crate) async fn sent(
    router: axum::Router,
    method: &str,
    path: &str,
    secret: &str,
    from: Option<(IpAddr, bool)>,
    body: &str,
) -> (Answer, HeaderMap) {
    let mut building = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(TOKEN_HEADER, secret);
    for (name, value) in from_here() {
        building = building.header(name, value);
    }
    let Ok(mut request) = building.body(Body::from(body.to_owned())) else {
        unreachable!("the request a test writes is one that can be built")
    };
    if let Some((from, encrypted)) = from {
        request.extensions_mut().insert(Arrived { from, encrypted });
    }
    let Ok(response) = router.oneshot(request).await;
    let status = response.status();
    let headers = response.headers().clone();
    let left = headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let Ok(read) = to_bytes(response.into_body(), 64 * 1024).await else {
        unreachable!("an answer this surface produces is one that can be read")
    };
    let answer = Answer {
        status,
        body: String::from_utf8_lossy(&read).into_owned(),
        left,
    };
    (answer, headers)
}

/// The refusal code an answer carries.
pub(crate) fn code(answer: &Answer) -> String {
    serde_json::from_str::<serde_json::Value>(&answer.body)
        .ok()
        .and_then(|body| {
            body.pointer("/data/code")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn a_read_key_from_this_machine_reads() {
    let keys = keeping_keys("reads");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    let read = presented(
        router,
        "GET",
        "/api/explain?word=indexer",
        keys.read.as_str(),
        Some((HERE, false)),
    )
    .await;
    assert_eq!(read.status, StatusCode::OK, "{}", read.body);
}

#[tokio::test]
async fn a_key_from_another_machine_is_read_only_over_tls() {
    let keys = keeping_keys("over-tls");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    let plain = presented(
        router.clone(),
        "GET",
        "/api/explain?word=indexer",
        keys.read.as_str(),
        Some((NEXT_DOOR, false)),
    )
    .await;
    assert_eq!(plain.status, StatusCode::FORBIDDEN);
    assert_eq!(code(&plain), "ADMIT-11");
    let encrypted = presented(
        router,
        "GET",
        "/api/explain?word=indexer",
        keys.read.as_str(),
        Some((NEXT_DOOR, true)),
    )
    .await;
    assert_eq!(encrypted.status, StatusCode::OK, "{}", encrypted.body);
}

#[tokio::test]
async fn a_key_over_a_connection_nobody_placed_is_refused_before_it_is_looked_at() {
    let keys = keeping_keys("unplaced");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    let refused = presented(
        router,
        "GET",
        "/api/explain?word=indexer",
        keys.read.as_str(),
        None,
    )
    .await;
    assert_eq!(code(&refused), "ADMIT-11");
}

#[tokio::test]
async fn a_revoked_or_unknown_key_is_refused_exactly_as_a_wrong_token_is() {
    let keys = keeping_keys("as-a-wrong-token");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    let wrong_token = presented(
        router.clone(),
        "GET",
        "/api/explain?word=indexer",
        &"0f".repeat(32),
        Some((HERE, false)),
    )
    .await;
    let revoked = presented(
        router.clone(),
        "GET",
        "/api/explain?word=indexer",
        keys.revoked.as_str(),
        Some((HERE, false)),
    )
    .await;
    let unknown = presented(
        router,
        "GET",
        "/api/explain?word=indexer",
        a_secret(9).as_str(),
        Some((HERE, false)),
    )
    .await;
    for refused in [&revoked, &unknown] {
        assert_eq!(refused.status, wrong_token.status);
        assert_eq!(refused.body, wrong_token.body);
    }
    assert_eq!(code(&wrong_token), "ADMIT-4");
}

#[tokio::test]
async fn a_read_key_is_refused_every_action_naming_its_scope() {
    let keys = keeping_keys("read-acts");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    let refused = presented(
        router,
        "POST",
        "/api/actions/downloads-pause",
        keys.read.as_str(),
        Some((HERE, false)),
    )
    .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(code(&refused), "ADMIT-12");
    assert!(refused.body.contains("scope read"), "{}", refused.body);
}

#[tokio::test]
async fn an_act_key_is_refused_what_no_key_may_call() {
    let keys = keeping_keys("act-refused");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    for action in ["seed", "reset", "forget", "backup"] {
        let refused = presented(
            router.clone(),
            "POST",
            &format!("/api/actions/{action}"),
            keys.act.as_str(),
            Some((HERE, false)),
        )
        .await;
        assert_eq!(code(&refused), "ADMIT-12", "{action}: {}", refused.body);
        assert!(refused.body.contains("scope act"), "{}", refused.body);
    }
}

#[tokio::test]
async fn an_act_key_is_let_through_to_what_a_key_may_call() {
    let keys = keeping_keys("act-called");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    let called = presented(
        router,
        "POST",
        "/api/actions/downloads-pause",
        keys.act.as_str(),
        Some((HERE, false)),
    )
    .await;
    assert_ne!(called.status, StatusCode::FORBIDDEN, "{}", called.body);
}

#[tokio::test]
async fn no_key_may_mint_list_or_revoke_keys_whatever_its_scope() {
    let keys = keeping_keys("about-keys");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    for secret in [&keys.read, &keys.act] {
        for (method, path) in [
            ("GET", "/api/keys"),
            ("POST", "/api/keys"),
            ("DELETE", "/api/keys/reader"),
        ] {
            let refused = presented(
                router.clone(),
                method,
                path,
                secret.as_str(),
                Some((HERE, false)),
            )
            .await;
            assert_eq!(
                code(&refused),
                "ADMIT-12",
                "{method} {path}: {}",
                refused.body
            );
        }
    }
    let member = presented(
        router,
        "GET",
        "/api/keys",
        keys.member.as_str(),
        Some((HERE, false)),
    )
    .await;
    assert_eq!(member.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_member_key_is_that_member_and_nothing_more() {
    let keys = keeping_keys("member");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    let theirs = presented(
        router.clone(),
        "GET",
        "/api/explain?word=indexer",
        keys.member.as_str(),
        Some((HERE, false)),
    )
    .await;
    assert_eq!(code(&theirs), "ADMIT-6", "{}", theirs.body);
    let mut carried = from_here();
    carried.push((TOKEN_HEADER, keys.member.as_str().to_owned()));
    let arrived = Arrived {
        from: HERE,
        encrypted: false,
    };
    let (status, kinds) = listened(router, &carried, Some(arrived), 3).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a member key was not let on to its stream"
    );
    assert_eq!(
        kinds,
        a_members_kinds(),
        "a member key heard more than its member's own: {kinds:?}"
    );
}

/// The frames a member key's stream says, opened and then listened to until it ends
/// or `most` have been said, with `meanwhile` done once the first is heard.
async fn heard_until_it_ends(
    router: axum::Router,
    secret: &str,
    most: usize,
    meanwhile: impl FnOnce(),
) -> usize {
    use futures_util::StreamExt as _;

    let mut building = Request::builder()
        .method("GET")
        .uri(lemonfiber_api::events::PATH)
        .header(TOKEN_HEADER, secret);
    for (name, value) in from_here() {
        building = building.header(name, value);
    }
    let Ok(mut request) = building.body(Body::empty()) else {
        unreachable!("the request a test writes is one that can be built")
    };
    request.extensions_mut().insert(Arrived {
        from: HERE,
        encrypted: false,
    });
    let Ok(response) = router.oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response.into_body().into_data_stream();
    let _first = body.next().await;
    meanwhile();
    let mut said = 0;
    while said < most && body.next().await.is_some() {
        said += 1;
    }
    said
}

/// How many more things a stream that should have ended is let say before it is
/// taken as one that never will: a beat and a reading either side of the next asking.
const BEFORE_IT_ENDS: usize = 8;

#[tokio::test(start_paused = true)]
async fn a_member_keys_stream_ends_once_the_key_is_revoked() {
    let keys = keeping_keys("member-stream-revoked");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    let said = heard_until_it_ends(router, keys.member.as_str(), BEFORE_IT_ENDS, || {
        let Ok(mut kept) = Kept::at(&keys.kept) else {
            unreachable!("the keys this test kept can be read back")
        };
        for record in &mut kept.keys {
            if matches!(record.scope, Scope::Member { .. }) {
                record.revoked = Some("2026-10-05T08:00:00".to_owned());
            }
        }
        assert!(kept.keep(&keys.kept).is_ok());
    })
    .await;
    assert!(
        said < BEFORE_IT_ENDS,
        "a revoked key went on hearing its member's stream"
    );
}

#[tokio::test(start_paused = true)]
async fn a_member_keys_stream_ends_once_the_key_no_longer_names_the_member() {
    let keys = keeping_keys("member-stream-rescoped");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    let said = heard_until_it_ends(router, keys.member.as_str(), BEFORE_IT_ENDS, || {
        let Ok(mut kept) = Kept::at(&keys.kept) else {
            unreachable!("the keys this test kept can be read back")
        };
        for record in &mut kept.keys {
            if matches!(record.scope, Scope::Member { .. }) {
                record.scope = Scope::Read;
            }
        }
        assert!(kept.keep(&keys.kept).is_ok());
    })
    .await;
    assert!(
        said < BEFORE_IT_ENDS,
        "a key that no longer names the member went on hearing their stream"
    );
}

#[tokio::test(start_paused = true)]
async fn a_member_keys_stream_ends_once_the_member_has_left() {
    let keys = keeping_keys("member-stream-left");
    let household = AHousehold::knowing("a7f3");
    let (router, _) = surface_with(&keys, Arc::clone(&household));
    let said = heard_until_it_ends(router, keys.member.as_str(), BEFORE_IT_ENDS, || {
        household.withdraw();
    })
    .await;
    assert!(
        said < BEFORE_IT_ENDS,
        "a member who left went on hearing their stream"
    );
}

#[tokio::test]
async fn a_member_key_whose_account_has_left_is_refused_as_nobody() {
    let keys = keeping_keys("member-gone");
    let household = AHousehold::knowing("a7f3");
    household.withdraw();
    let (router, _) = surface_with(&keys, household);
    let refused = presented(
        router,
        "GET",
        "/api/requests",
        keys.member.as_str(),
        Some((HERE, false)),
    )
    .await;
    assert_eq!(code(&refused), "ADMIT-4");
}

#[tokio::test]
async fn a_member_key_is_unconfirmed_while_the_household_cannot_be_asked() {
    let keys = keeping_keys("member-dark");
    let household = AHousehold::knowing("a7f3");
    household.go_dark();
    let (router, admitting) = surface_with(&keys, household);
    let refused = presented(
        router,
        "GET",
        "/api/requests",
        keys.member.as_str(),
        Some((NEXT_DOOR, true)),
    )
    .await;
    assert_eq!(code(&refused), "ADMIT-7", "{}", refused.body);
    // A key the household could not be asked about was no guess, so it is not counted.
    assert!(admitting
        .attempts
        .waiting(NEXT_DOOR, moment())
        .await
        .is_none());
}

#[tokio::test]
async fn wrong_keys_from_many_machines_at_once_meet_the_shared_wait() {
    let keys = keeping_keys("many-machines");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    let mut held = None;
    for last in 1..=40u8 {
        let answer = presented(
            router.clone(),
            "GET",
            "/api/explain?word=indexer",
            a_secret(last).as_str(),
            Some((IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 2, last)), true)),
        )
        .await;
        if answer.status == StatusCode::TOO_MANY_REQUESTS {
            held = Some(last);
            break;
        }
    }
    // Each machine guessed once, which earns none of them a wait of its own: what
    // stopped them is the pool every guess draws on.
    assert!(held.is_some_and(|last| last > 3));
}

#[tokio::test]
async fn wrong_keys_earn_the_wait_wrong_passwords_do_and_a_right_key_meets_it_too() {
    let keys = keeping_keys("guessed");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    let mut held = None;
    for guess in 0..12u8 {
        let answer = presented(
            router.clone(),
            "GET",
            "/api/explain?word=indexer",
            a_secret(100 + guess).as_str(),
            Some((NEXT_DOOR, true)),
        )
        .await;
        if answer.status == StatusCode::TOO_MANY_REQUESTS {
            held = Some(answer);
            break;
        }
    }
    let Some(held) = held else {
        unreachable!("twelve wrong keys in a moment earned no wait")
    };
    assert!(held.left.is_some());
    // While the wait holds, a right key is not even looked at.
    let right = presented(
        router,
        "GET",
        "/api/explain?word=indexer",
        keys.read.as_str(),
        Some((NEXT_DOOR, true)),
    )
    .await;
    assert_eq!(right.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn a_revoked_key_presented_again_and_again_earns_no_wait() {
    let keys = keeping_keys("revoked-again");
    let (router, admitting) = surface_with(&keys, AHousehold::knowing("a7f3"));
    for _ in 0..20 {
        let refused = presented(
            router.clone(),
            "GET",
            "/api/explain?word=indexer",
            keys.revoked.as_str(),
            Some((NEXT_DOOR, true)),
        )
        .await;
        assert_eq!(code(&refused), "ADMIT-4");
    }
    assert!(admitting
        .attempts
        .waiting(NEXT_DOOR, moment())
        .await
        .is_none());
}

#[tokio::test]
async fn a_member_key_whose_account_has_left_earns_no_wait() {
    let keys = keeping_keys("member-gone-again");
    let household = AHousehold::knowing("a7f3");
    household.withdraw();
    let (router, admitting) = surface_with(&keys, household);
    for _ in 0..20 {
        let refused = presented(
            router.clone(),
            "GET",
            "/api/requests",
            keys.member.as_str(),
            Some((NEXT_DOOR, true)),
        )
        .await;
        assert_eq!(code(&refused), "ADMIT-4");
    }
    assert!(admitting
        .attempts
        .waiting(NEXT_DOOR, moment())
        .await
        .is_none());
}

#[tokio::test]
async fn a_right_key_forgives_nothing_but_itself() {
    let keys = keeping_keys("forgives-nothing");
    let (router, admitting) = surface_with(&keys, AHousehold::knowing("a7f3"));
    let guess = |byte: u8| {
        let router = router.clone();
        async move {
            presented(
                router,
                "GET",
                "/api/explain?word=indexer",
                a_secret(byte).as_str(),
                Some((HERE, false)),
            )
            .await
        }
    };
    let _ = guess(150).await;
    let _ = guess(151).await;
    let right = presented(
        router.clone(),
        "GET",
        "/api/explain?word=indexer",
        keys.read.as_str(),
        Some((HERE, false)),
    )
    .await;
    assert_eq!(right.status, StatusCode::OK, "{}", right.body);
    assert!(admitting.attempts.waiting(HERE, moment()).await.is_none());
    // Two more wrong ones are the fourth and fifth, not the first and second.
    let _ = guess(152).await;
    let _ = guess(153).await;
    assert!(admitting.attempts.waiting(HERE, moment()).await.is_some());
}

#[tokio::test]
async fn a_use_is_written_down_and_the_secret_is_not() {
    let keys = keeping_keys("used");
    let (router, _) = surface_with(&keys, AHousehold::knowing("a7f3"));
    let _ = presented(
        router,
        "GET",
        "/api/explain?word=indexer",
        keys.read.as_str(),
        Some((HERE, false)),
    )
    .await;
    let used = fs::read_to_string(&keys.used).unwrap_or_default();
    assert!(used.contains("reader"), "{used}");
    assert!(!used.contains(keys.read.as_str()));
    let kept = fs::read_to_string(&keys.kept).unwrap_or_default();
    assert!(!kept.contains(keys.read.as_str()));
}
