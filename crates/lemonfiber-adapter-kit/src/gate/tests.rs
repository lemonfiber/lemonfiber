use axum::http::StatusCode;
use lemonfiber_contract::adapter::{KEY_LIMIT, REQUEST_KEY_FILE};
use lemonfiber_contract::wire;

use super::{admits, KEYS_MOST};
use crate::tests::{asked, kit, KEY};

const GATE: &str = "the-gates-key";
const ROTATED: &str = "the-gates-next-key";

/// A directory of its own for `name`, holding the request gate's keys as `written`.
fn written(name: &str, keys: Option<&str>) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("adapter-kit-gate-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    if let Some(keys) = keys {
        let _ = std::fs::write(dir.join(REQUEST_KEY_FILE), keys);
    }
    dir
}

fn refusal(body: &str) -> Option<wire::Kind> {
    serde_json::from_str::<wire::Refusal>(body)
        .ok()
        .map(|refusal| refusal.kind)
}

#[test]
fn the_gate_may_ask_a_curators_requests_and_nothing_else() {
    for operation in [
        "catalogued",
        "titles",
        "held_title",
        "request",
        "search",
        "remove",
        "downloading",
        "quality_profiles",
        "root_folders",
        "lookup",
    ] {
        assert!(admits("library.curate", "v1", operation), "{operation}");
        assert!(!admits("library.curate", "v2", operation), "{operation}");
        assert!(!admits("media.serve", "v1", operation), "{operation}");
    }
    for operation in [
        "register_download_client",
        "set_hardlinks",
        "identity",
        "carry",
    ] {
        assert!(!admits("library.curate", "v1", operation), "{operation}");
    }
}

#[tokio::test]
async fn a_request_under_the_gates_key_reaches_an_operation_it_may_ask() {
    let dir = written("reaches", Some(&format!("{GATE}\n\n  {ROTATED}  \n")));
    for key in [GATE, ROTATED] {
        let (status, _, body) = asked(
            kit().asked_by_the_gate(&dir),
            "POST",
            "/lemonfiber/library.curate/v1/titles",
            Some(key),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{key}: {body}");
        assert_eq!(refusal(&body), Some(wire::Kind::UnknownOperation), "{body}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn the_gates_key_opens_nothing_else_and_says_so() {
    let dir = written("refused", Some(GATE));
    for (method, path) in [
        ("POST", "/lemonfiber/library.curate/v1/set_hardlinks"),
        ("POST", "/lemonfiber/subtitles.fetch/v1/watching"),
        ("GET", "/lemonfiber/adapter/v1/about"),
        ("GET", "/lemonfiber/adapter/v1/ready"),
    ] {
        let (status, sort, body) =
            asked(kit().asked_by_the_gate(&dir), method, path, Some(GATE)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(sort, "application/problem+json", "{path}");
        assert_eq!(refusal(&body), Some(wire::Kind::Disallowed), "{path}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn the_plugins_key_still_opens_everything_beside_the_gates() {
    let dir = written("core", Some(GATE));
    let (status, _, _) = asked(
        kit().asked_by_the_gate(&dir),
        "POST",
        "/lemonfiber/subtitles.fetch/v1/watching",
        Some(KEY),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn no_key_the_core_did_not_write_for_the_gate_is_the_gates() {
    let oversized = format!(
        "{GATE}\n{}",
        "k".repeat(usize::try_from(KEYS_MOST).unwrap_or(1024))
    );
    let unwritten = written("none", None);
    let too_large = written("large", Some(&oversized));
    let blank = written("blank", Some("\n \n"));
    for (dir, key) in [
        (&unwritten, GATE),
        (&too_large, GATE),
        (&blank, ""),
        (&blank, " "),
    ] {
        let (status, _, _) = asked(
            kit().asked_by_the_gate(dir),
            "POST",
            "/lemonfiber/library.curate/v1/titles",
            Some(key),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{dir:?} {key:?}");
    }
    let (status, _, _) = asked(
        kit(),
        "POST",
        "/lemonfiber/library.curate/v1/titles",
        Some(GATE),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(KEYS_MOST >= 2 * KEY_LIMIT as u64);
    for dir in [unwritten, too_large, blank] {
        let _ = std::fs::remove_dir_all(dir);
    }
}
