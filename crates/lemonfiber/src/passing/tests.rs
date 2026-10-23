use std::process::ExitCode;

use lemonfiber_core::pass::{File, Key, Live, LivePath, FORWARDED, KEY_BYTES, PATH};
use lemonfiber_fixtures::scratch::Scratch;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use super::{answering, held, serve};

/// The key these passes are made with.
const fn key() -> Key {
    Key::of([5; KEY_BYTES])
}

/// A live stream's id, in the shape the paths below declare.
const STREAM: &str = "abc123";

/// A configuration directory holding the key and the live-stream paths.
fn configured(name: &str) -> Scratch {
    let scratch = Scratch::new(name);
    let live = Live::of(vec![LivePath {
        pattern: "^/live/(?P<stream>[^/]+)/".to_owned(),
        stream: "^[a-z0-9]{6}$".to_owned(),
    }]);
    let _ = std::fs::write(scratch.path().join(File::Key.name()), key().written());
    let _ = std::fs::write(scratch.path().join(File::Live.name()), live.written());
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
    let _ = std::fs::write(config.path().join(File::Live.name()), "{}");
    let unreadable_live = held(config.path()).err().unwrap_or_default();
    let _ = std::fs::write(config.path().join(File::Key.name()), "short");
    let unreadable_key = held(config.path()).err().unwrap_or_default();
    let unmatchable = configured("pass-unmatchable");
    let _ = std::fs::write(
        unmatchable.path().join(File::Live.name()),
        Live::of(vec![LivePath {
            pattern: "^/live/(".to_owned(),
            stream: "^.+$".to_owned(),
        }])
        .written(),
    );

    assert!(
        unreadable_live.contains(File::Live.name()),
        "{unreadable_live}"
    );
    assert!(
        unreadable_key.contains(File::Key.name()),
        "{unreadable_key}"
    );
    assert!(held(unmatchable.path())
        .err()
        .is_some_and(|why| why.contains("cannot be matched with")));
}
