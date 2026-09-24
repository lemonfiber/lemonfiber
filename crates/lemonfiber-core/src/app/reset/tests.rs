use std::path::{Path, PathBuf};

use include_dir::{include_dir, Dir};

use super::reset;
use crate::app::Ctx;
use crate::config::Settings;
use crate::platform::Environment;
use crate::stack::Source;
use crate::test_support::a_context;

static STACKLET: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/stacklet");

/// A directory of this test's own, and the env-file path within it.
fn scratch(name: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("lemonfiber-reset-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    (dir.join("stack"), dir.join(".env"))
}

/// A context operating the embedded fixture stack, materialised under `into`.
fn ctx(into: Option<PathBuf>, env: Option<PathBuf>) -> Ctx {
    a_context()
        .over(Source::Embedded(&STACKLET))
        .settings(Settings {
            stack_dir: into,
            env_file: env,
            ..Settings::default()
        })
        .environment(Environment::LinuxNative)
        .build()
}

#[tokio::test]
async fn a_confirmed_reset_reverts_an_edited_file_to_lemonfibers() {
    let (into, env) = scratch("confirm");
    // Materialise, then the operator edits a file.
    let _ = reset(&ctx(Some(into.clone()), Some(env.clone())), true).await;
    let edited = "services:\n  sonarr:\n    image: my-own\n";
    let _ = std::fs::write(into.join("compose.yaml"), edited);

    let report = reset(&ctx(Some(into.clone()), Some(env)), true)
        .await
        .unwrap_or_default();
    assert!(report.confirmed);
    assert!(
        report
            .reverted
            .iter()
            .any(|edit| edit.path == "compose.yaml"),
        "the reverted edit is named"
    );
    // The edit is gone — the file is lemonfiber's own again.
    assert_ne!(
        std::fs::read_to_string(into.join("compose.yaml")).unwrap_or_default(),
        edited
    );
}

#[tokio::test]
async fn an_unconfirmed_reset_previews_and_writes_nothing() {
    let (into, env) = scratch("preview");
    let _ = reset(&ctx(Some(into.clone()), Some(env.clone())), true).await;
    let edited = "services:\n  sonarr:\n    image: my-own\n";
    let _ = std::fs::write(into.join("compose.yaml"), edited);

    let report = reset(&ctx(Some(into.clone()), Some(env)), false)
        .await
        .unwrap_or_default();
    assert!(!report.confirmed);
    assert!(report
        .reverted
        .iter()
        .any(|edit| edit.path == "compose.yaml"));
    // The preview touched nothing.
    assert_eq!(
        std::fs::read_to_string(into.join("compose.yaml")).unwrap_or_default(),
        edited
    );
}

#[tokio::test]
async fn a_reset_with_nowhere_to_write_is_an_error() {
    // An embedded stack with no directory to materialise into cannot be reset.
    assert!(reset(&ctx(None, None), true).await.is_err());
}

#[tokio::test]
async fn a_reset_over_an_unreadable_choice_is_an_error() {
    let (into, env) = scratch("bad-choice");
    // A present-but-corrupt recorded choice cannot be read, so the reset stops rather
    // than guessing the state it would restore.
    let _ = std::fs::write(Path::new(&env).with_file_name("quality.json"), "not json");
    assert!(reset(&ctx(Some(into), Some(env)), true).await.is_err());
}
