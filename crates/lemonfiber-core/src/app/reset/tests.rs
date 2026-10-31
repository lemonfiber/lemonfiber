use std::path::{Path, PathBuf};

use super::reset;
use crate::app::Ctx;
use crate::config::Settings;
use crate::platform::Environment;
use crate::stack::Source;
use crate::test_support::{a_context, overlaid};

/// A reset answered the way a surface answers one: read, then the offer it printed.
async fn confirmed(ctx: &Ctx) -> Result<crate::model::ResetReport, Box<crate::error::Problem>> {
    let offer = reset(ctx, None).await?.offer;
    reset(ctx, Some(offer)).await
}

/// A directory of this test's own, and the env-file path within it.
fn scratch(name: &str) -> (lemonfiber_fixtures::scratch::Scratch, PathBuf) {
    let into = lemonfiber_fixtures::scratch::Scratch::new(name).within("stack");
    let env = into.with_file_name(".env");
    (into, env)
}

/// A context operating the embedded fixture stack, materialised under `into`.
fn ctx(into: Option<PathBuf>, env: Option<PathBuf>) -> Ctx {
    a_context()
        .over(Source::Embedded(&crate::test_support::STACKLET))
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

/// A context operating the stack this build assembles from its bundle, materialised under
/// `into`.
fn bundled(into: &Path, env: &Path) -> Ctx {
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
    let _ = confirmed(&carried(&into, &env)).await;
    let project = into.join(crate::project::FILE);
    let _ = std::fs::write(&project, EDITED);

    let report = confirmed(&carried(&into, &env)).await.unwrap_or_default();

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
    let _ = confirmed(&carried(&into, &env)).await;
    let project = into.join(crate::project::FILE);
    let _ = std::fs::write(&project, EDITED);

    let report = reset(&carried(&into, &env), None).await.unwrap_or_default();

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
async fn an_answer_to_a_reading_that_has_moved_is_refused_and_reverts_nothing() {
    let (into, env) = scratch("moved");
    let _ = confirmed(&carried(&into, &env)).await;
    let project = into.join(crate::project::FILE);
    let _ = std::fs::write(&project, EDITED);
    let read = reset(&carried(&into, &env), None)
        .await
        .unwrap_or_default()
        .offer;
    let later = format!("{EDITED}  radarr:\n    image: mine-too\n");
    let _ = std::fs::write(&project, &later);

    let refused = reset(&carried(&into, &env), Some(read)).await.err();

    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(crate::error::codes::stack::RESET_MOVED)
    );
    assert_eq!(refused.map(|problem| problem.status()), Some(400));
    assert_eq!(std::fs::read_to_string(&project).unwrap_or_default(), later);
}

#[tokio::test]
async fn a_reset_with_nowhere_to_write_is_an_error() {
    // An embedded stack with no directory to materialise into cannot be reset.
    assert!(confirmed(&ctx(None, None)).await.is_err());
}

#[tokio::test]
async fn a_reset_over_an_unreadable_choice_is_an_error() {
    let (into, env) = scratch("bad-choice");
    // A present-but-corrupt recorded choice cannot be read, so the reset stops rather
    // than guessing the state it would restore.
    let _ = std::fs::write(Path::new(&env).with_file_name("quality.json"), "not json");
    assert!(confirmed(&ctx(Some(into.to_path_buf()), Some(env)))
        .await
        .is_err());
}

#[tokio::test]
async fn a_reset_over_a_stack_that_does_not_pass_its_checks_is_an_error() {
    let (into, env) = scratch("unchecked");

    let refused = confirmed(&ctx(Some(into.to_path_buf()), Some(env)))
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

    assert!(confirmed(&carried(&into, &env)).await.is_err());
}

/// The overlay the operator wrote is journalled, by content, by a confirmed reset, which
/// writes the project, and by no preview.
#[tokio::test]
async fn a_confirmed_reset_journals_the_overlay_and_a_preview_does_not() {
    let (into, env) = scratch("overlaid");
    let overlay = crate::overlay::at(&env);
    let _ = std::fs::create_dir_all(overlay.parent().unwrap_or(&into));
    let _ = std::fs::write(&overlay, EDITED);

    let _ = reset(&carried(&into, &env), None).await;
    let previewed = overlaid(&carried(&into, &env));
    let _ = confirmed(&carried(&into, &env)).await;

    assert_eq!(previewed, []);
    assert_eq!(
        overlaid(&carried(&into, &env)),
        [(None, Some(crate::credential::fingerprint(EDITED)))]
    );
}

/// A reset over a data root the setting names that is refused writes nothing and is
/// refused naming it.
#[tokio::test]
async fn a_reset_over_a_refused_data_root_is_refused_and_writes_nothing() {
    let (into, env) = scratch("reset-rooted");
    assert!(
        crate::config::store::set(&env, crate::config::DATA_ROOT_KEY, "nfs://nas local/x").is_ok()
    );

    let refused = confirmed(&carried(&into, &env))
        .await
        .err()
        .map(|problem| problem.code);

    assert_eq!(refused, Some(crate::error::codes::storage::ROOT_REFUSED));
    assert!(!into.join(crate::project::FILE).exists());
}

/// A confirmed reset writes each secrets file the project names, and a preview none.
#[tokio::test]
async fn a_confirmed_reset_writes_each_secrets_file_and_a_preview_none() {
    let (into, env) = scratch("unfiled");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    let guard = env
        .with_file_name(crate::config::paths::SECRETS_FILES)
        .join("gluetun.env");

    assert!(reset(&bundled(&into, &env), None).await.is_ok());
    let previewed = guard.exists();
    assert!(confirmed(&bundled(&into, &env)).await.is_ok());

    assert!(!previewed);
    assert!(guard.is_file());
}

/// A reset whose record of installed plugins cannot be read is refused.
#[tokio::test]
async fn a_reset_whose_register_cannot_be_read_is_refused() {
    let (into, env) = scratch("unregistered");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    let _ = std::fs::write(
        env.with_file_name(crate::config::paths::PLUGINS),
        "not a register",
    );

    assert!(confirmed(&carried(&into, &env)).await.is_err());
    assert!(!into.join(crate::project::FILE).exists());
}

/// A confirmed reset that cannot write a secrets file the project names is an error.
#[tokio::test]
async fn a_confirmed_reset_that_cannot_write_a_secrets_file_is_an_error() {
    let (into, env) = scratch("unfilable");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    let _ = std::fs::write(
        env.with_file_name(crate::config::paths::SECRETS_FILES),
        "a file, not a directory",
    );

    assert!(confirmed(&bundled(&into, &env)).await.is_err());
}
