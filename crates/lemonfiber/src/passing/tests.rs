use std::process::ExitCode;

use lemonfiber_core::pass::{
    renewed_as, Confirmed, File, ItemPath, Key, LivePath, Paths, Renewals, Renewed, Session,
    FORWARDED, KEY_BYTES, NONCE_BYTES, PATH, SESSION,
};
use lemonfiber_fixtures::scratch::Scratch;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use super::{answered, answering, compiled, serve, texts, Held};

/// The key and the live-stream paths under `config`, as the service compiles them.
fn held(config: &std::path::Path) -> Result<Held, String> {
    let texts = texts(config)?;
    compiled(&texts.key, &texts.paths, texts.renewals.as_deref())
}

/// The key these passes are made with.
const fn key() -> Key {
    Key::of([5; KEY_BYTES])
}

/// A live stream's id, in the shape the paths below declare.
const STREAM: &str = "abc123";

/// An item's id, in the shape the paths below declare.
const ITEM: &str = "0123456789abcdef0123456789abcdef";

/// A configuration directory holding the key and the live-stream paths.
fn configured(name: &str) -> Scratch {
    let scratch = Scratch::new(name);
    let live = Paths::of(
        vec![LivePath {
            pattern: "^/live/(?P<stream>[^/]+)/".to_owned(),
            parameter: None,
            stream: "^[a-z0-9]{6}$".to_owned(),
        }],
        vec![ItemPath {
            pattern: "^/videos/(?P<item>[^/]+)/".to_owned(),
            query: None,
            item: "^[0-9a-f]{32}$".to_owned(),
        }],
    );
    let _ = std::fs::write(scratch.path().join(File::Key.name()), key().written());
    let _ = std::fs::write(scratch.path().join(File::Paths.name()), live.written());
    scratch
}

/// The status line the service answers `forwarded` with, at `at`.
async fn asked(at: std::net::SocketAddr, forwarded: Option<&str>) -> String {
    let Ok(mut stream) = tokio::net::TcpStream::connect(at).await else {
        return String::new();
    };
    let header = forwarded.map_or_else(String::new, |uri| format!("{FORWARDED}: {uri}\r\n"));
    let request = format!("GET {PATH} HTTP/1.1\r\nHost: pass\r\n{header}Connection: close\r\n\r\n");
    let _ = stream.write_all(request.as_bytes()).await;
    let mut answer = String::new();
    let _ = stream.read_to_string(&mut answer).await;
    answer.lines().next().unwrap_or_default().to_owned()
}

/// A free address on this machine.
async fn free() -> std::net::SocketAddr {
    let bound = tokio::net::TcpListener::bind("127.0.0.1:0").await;
    let at = bound
        .as_ref()
        .ok()
        .and_then(|bound| bound.local_addr().ok());
    drop(bound);
    at.unwrap_or_else(|| std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
}

#[tokio::test]
async fn a_good_pass_is_answered_204_and_anything_else_403() {
    let config = configured("pass-answers");
    let at = free().await;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let path = config.path().to_path_buf();
    let serving = tokio::spawn(async move {
        serve(&path, at, Box::pin(async move { drop(stopped.await) })).await
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let pass = key().issue(STREAM, "member-1", now);

    let good = asked(at, Some(&format!("/live/{STREAM}/s.ts?lfpass={pass}"))).await;
    let elsewhere = asked(at, Some(&format!("/live/zzz999/s.ts?lfpass={pass}"))).await;
    let unnamed = asked(at, None).await;

    let _ = stop.send(());
    assert!(good.contains("204"), "{good}");
    assert!(elsewhere.contains("403"), "{elsewhere}");
    assert!(unnamed.contains("403"), "{unnamed}");
    assert_eq!(
        serving.await.map(|code| code == ExitCode::SUCCESS).ok(),
        Some(true)
    );
}

#[tokio::test]
async fn a_service_missing_its_files_or_its_address_does_not_start() {
    let empty = Scratch::new("pass-unconfigured");
    let taken = tokio::net::TcpListener::bind("127.0.0.1:0").await;
    let at = taken
        .as_ref()
        .ok()
        .and_then(|bound| bound.local_addr().ok())
        .unwrap_or_else(|| std::net::SocketAddr::from(([127, 0, 0, 1], 0)));
    let config = configured("pass-address-taken");

    let unconfigured = answering(empty.path(), at).await;
    let unbound = serve(config.path(), at, Box::pin(std::future::pending())).await;

    assert_eq!(unconfigured, ExitCode::FAILURE);
    assert_eq!(unbound, ExitCode::FAILURE);
    drop(taken);
}

#[test]
fn files_the_service_cannot_read_are_named() {
    let config = configured("pass-unreadable");
    let _ = std::fs::write(config.path().join(File::Paths.name()), "{}");
    let unreadable_live = held(config.path()).err().unwrap_or_default();
    let _ = std::fs::write(config.path().join(File::Key.name()), "short");
    let unreadable_key = held(config.path()).err().unwrap_or_default();
    let unmatchable = configured("pass-unmatchable");
    let _ = std::fs::write(
        unmatchable.path().join(File::Paths.name()),
        Paths::of(
            vec![LivePath {
                pattern: "^/live/(".to_owned(),
                parameter: None,
                stream: "^.+$".to_owned(),
            }],
            Vec::new(),
        )
        .written(),
    );

    assert!(
        unreadable_live.contains(File::Paths.name()),
        "{unreadable_live}"
    );
    assert!(
        unreadable_key.contains(File::Key.name()),
        "{unreadable_key}"
    );
    assert!(held(unmatchable.path())
        .err()
        .is_some_and(|why| why.contains("cannot be matched with")));
    let renewals_unreadable = configured("pass-renewals-unreadable");
    let _ = std::fs::create_dir(renewals_unreadable.path().join(File::Renewals.name()));
    assert!(held(renewals_unreadable.path())
        .err()
        .is_some_and(|why| why.contains(File::Renewals.name())));
}

#[tokio::test]
async fn files_the_core_rewrites_hold_from_the_next_question() {
    let config = configured("pass-rewritten");
    let at = free().await;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let path = config.path().to_path_buf();
    let serving = tokio::spawn(async move {
        serve(&path, at, Box::pin(async move { drop(stopped.await) })).await
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let pass = key().issue(STREAM, "member-1", now);
    let moved = format!("/watch/{STREAM}/s.ts?lfpass={pass}");

    let before = asked(at, Some(&moved)).await;
    let _ = std::fs::write(
        config.path().join(File::Paths.name()),
        Paths::of(
            vec![LivePath {
                pattern: "^/watch/(?P<stream>[^/]+)/".to_owned(),
                parameter: None,
                stream: "^[a-z0-9]{6}$".to_owned(),
            }],
            Vec::new(),
        )
        .written(),
    );
    let after = asked(at, Some(&moved)).await;
    let again = asked(at, Some(&moved)).await;
    let _ = std::fs::write(config.path().join(File::Key.name()), "short");
    let unreadable = asked(at, Some(&moved)).await;

    let _ = stop.send(());
    assert!(before.contains("403"), "{before}");
    assert!(after.contains("204"), "{after}");
    assert!(again.contains("204"), "{again}");
    assert!(unreadable.contains("403"), "{unreadable}");
    drop(serving.await);
}

#[test]
fn an_item_pass_is_answered_with_its_session_in_the_one_header_and_a_live_pass_without() {
    let config = configured("pass-item");
    let Ok(held) = held(config.path()) else {
        unreachable!("the configured files compile");
    };
    let session = Session {
        member: "member-1".to_owned(),
        token: "ab".repeat(16),
    };
    let now = 1_800_000_000;
    let pass = key()
        .seal(ITEM, &session, [4; NONCE_BYTES], now)
        .unwrap_or_default();
    let asking = |forwarded: String| {
        let mut headers = axum::http::HeaderMap::new();
        if let Ok(value) = axum::http::HeaderValue::from_str(&forwarded) {
            headers.insert(FORWARDED, value);
        }
        answered(&held, &headers, now)
    };

    let opened = asking(format!("/videos/{ITEM}/master.m3u8?lfpass={pass}"));
    let elsewhere = asking(format!(
        "/videos/fedcba9876543210fedcba9876543210/master.m3u8?lfpass={pass}"
    ));
    let live = asking(format!(
        "/live/{STREAM}/s.ts?lfpass={}",
        key().issue(STREAM, "member-1", now)
    ));

    assert_eq!(opened.status(), 204);
    assert_eq!(
        opened
            .headers()
            .get(SESSION)
            .and_then(|value| value.to_str().ok()),
        Some(session.token.as_str())
    );
    assert!(opened
        .headers()
        .get(SESSION)
        .is_some_and(axum::http::HeaderValue::is_sensitive));
    assert_eq!(elsewhere.status(), 403);
    assert!(elsewhere.headers().get(SESSION).is_none());
    assert_eq!(live.status(), 204);
    assert!(live.headers().get(SESSION).is_none());
}

#[test]
fn an_item_pass_sealing_a_session_no_header_can_carry_is_refused() {
    let config = configured("pass-item-unsendable");
    let Ok(held) = held(config.path()) else {
        unreachable!("the configured files compile");
    };
    let session = Session {
        member: "member-1".to_owned(),
        token: "ab\ncd".to_owned(),
    };
    let now = 1_800_000_000;
    let pass = key()
        .seal(ITEM, &session, [4; NONCE_BYTES], now)
        .unwrap_or_default();
    let forwarded = format!("/videos/{ITEM}/master.m3u8?lfpass={pass}");
    let mut headers = axum::http::HeaderMap::new();
    if let Ok(value) = axum::http::HeaderValue::from_str(&forwarded) {
        headers.insert(FORWARDED, value);
    }

    let refused = answered(&held, &headers, now);

    assert_eq!(
        held.watching
            .confirms(&held.key, &forwarded, now, &held.renewals),
        Some(Confirmed::Item(session))
    );
    assert_eq!(refused.status(), 403);
    assert!(refused.headers().get(SESSION).is_none());
}

#[test]
fn a_renewal_the_core_wrote_holds_a_pass_and_an_unreadable_one_stops_the_service() {
    let config = configured("pass-renewed");
    let now = 1_800_000_000;
    let pass = key().issue(STREAM, "member-1", now);
    let mut renewals = Renewals::none();
    if let Some(name) = renewed_as(&pass) {
        renewals.passes.insert(
            name,
            Renewed {
                member: "member-1".to_owned(),
                lapses: now + 600,
            },
        );
    }
    let _ = std::fs::write(
        config.path().join(File::Renewals.name()),
        renewals.written(),
    );
    let later = now + 400;
    let asking = |held: &Held| {
        let mut headers = axum::http::HeaderMap::new();
        if let Ok(value) =
            axum::http::HeaderValue::from_str(&format!("/live/{STREAM}/s.ts?lfpass={pass}"))
        {
            headers.insert(FORWARDED, value);
        }
        answered(held, &headers, later).status()
    };

    assert_eq!(
        held(config.path()).map(|held| asking(&held)).ok(),
        Some(axum::http::StatusCode::NO_CONTENT)
    );
    let _ = std::fs::write(config.path().join(File::Renewals.name()), "{}");
    assert!(held(config.path()).is_err_and(|why| why.contains(File::Renewals.name())));
}
