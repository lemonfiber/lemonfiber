use std::path::{Path, PathBuf};

use include_dir::{include_dir, Dir};

use super::reset;
use crate::app::Ctx;
use crate::config::Settings;
use crate::platform::Environment;
use crate::stack::Source;
use crate::test_support::{a_context, overlaid};

static STACKLET: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/stacklet");

/// A directory of this test's own, and the env-file path within it.
fn scratch(name: &str) -> (lemonfiber_fixtures::scratch::Scratch, PathBuf) {
    let into = lemonfiber_fixtures::scratch::Scratch::new(name).within("stack");
    let env = into.with_file_name(".env");
    (into, env)
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

/// A context operating the stack this build carries, materialised under `into`.
fn carried(into: &Path, env: &Path) -> Ctx {
    a_context()
        .over(crate::test_support::embedded())
        .settings(Settings {
            stack_dir: Some(into.to_path_buf()),
            env_file: Some(env.to_path_buf()),
            ..Settings::default()
        })
        .environment(Environment::LinuxNative)
        .build()
}

/// What the operator wrote over the project.
const EDITED: &str = "services:\n  sonarr:\n    image: my-own\n";

#[tokio::test]
async fn a_confirmed_reset_reverts_an_edited_file_to_lemonfibers() {
    let (into, env) = scratch("confirm");
    let _ = reset(&carried(&into, &env), true).await;
    let project = into.join(crate::project::FILE);
    let _ = std::fs::write(&project, EDITED);

    let report = reset(&carried(&into, &env), true).await.unwrap_or_default();

    assert!(report.confirmed);
    assert!(
        report
            .reverted
            .iter()
            .any(|edit| edit.path == crate::project::FILE),
        "the reverted edit is named"
    );
    let written = std::fs::read_to_string(&project).unwrap_or_default();
    assert_ne!(written, EDITED);
    assert!(written.contains("\n  sonarr:\n"), "{written}");
}

#[tokio::test]
async fn an_unconfirmed_reset_previews_and_writes_nothing() {
    let (into, env) = scratch("preview");
    let _ = reset(&carried(&into, &env), true).await;
    let project = into.join(crate::project::FILE);
    let _ = std::fs::write(&project, EDITED);

    let report = reset(&carried(&into, &env), false)
        .await
        .unwrap_or_default();

    assert!(!report.confirmed);
    assert!(report
        .reverted
        .iter()
        .any(|edit| edit.path == crate::project::FILE));
    assert_eq!(
        std::fs::read_to_string(&project).unwrap_or_default(),
        EDITED
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
    assert!(reset(&ctx(Some(into.to_path_buf()), Some(env)), true)
        .await
        .is_err());
}

#[tokio::test]
async fn a_reset_over_a_stack_that_does_not_pass_its_checks_is_an_error() {
    let (into, env) = scratch("unchecked");

    let refused = reset(&ctx(Some(into.to_path_buf()), Some(env)), true)
        .await
        .err();

    assert!(
        refused.is_some_and(|problem| problem.code.as_str().starts_with("STACK-")),
        "refused by the stack's own checks"
    );
}

#[tokio::test]
async fn a_reset_whose_project_cannot_be_written_is_an_error() {
    let (into, env) = scratch("unwritable-project");
    let _ = std::fs::create_dir_all(into.join(crate::project::FILE));

    assert!(reset(&carried(&into, &env), true).await.is_err());
}

/// The overlay the operator wrote is journalled, by content, by a confirmed reset, which
/// writes the project, and by no preview.
#[tokio::test]
async fn a_confirmed_reset_journals_the_overlay_and_a_preview_does_not() {
    let (into, env) = scratch("overlaid");
    let overlay = crate::overlay::at(&env);
    let _ = std::fs::create_dir_all(overlay.parent().unwrap_or(&into));
    let _ = std::fs::write(&overlay, EDITED);

    let _ = reset(&carried(&into, &env), false).await;
    let previewed = overlaid(&carried(&into, &env));
    let _ = reset(&carried(&into, &env), true).await;

    assert_eq!(previewed, []);
    assert_eq!(
        overlaid(&carried(&into, &env)),
        [(None, Some(crate::credential::fingerprint(EDITED)))]
    );
}
