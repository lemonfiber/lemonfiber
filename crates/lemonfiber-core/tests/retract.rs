//! Putting back what the last repair changed, driven end to end.
//!
//! The reversal a repair earns. A change that lives inside a service goes back through
//! that service; a setting in lemonfiber's own environment file goes back on the host. The
//! two look alike in the journal and are reversed nothing alike, and getting that wrong
//! would write a service's field name into the environment file and report it restored.
//!
//! From here rather than a `#[cfg(test)]` module for the reason the other app-layer paths
//! are: the crate is compiled twice, and a path exercised only in-crate has its coverage
//! counted from the copy that never ran.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::files::Files;
use common::{Answer, Fake};
use lemonfiber_core::app::repair::retract;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::Settings;
use lemonfiber_core::journal::{Change, Kind};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::repair::OPERATION;
use lemonfiber_core::stack::Source;

/// The repository's own stack, so the targets resolve against a real description.
fn project() -> &'static Path {
    static PROJECT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    PROJECT
        .get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/media-stack"))
}

/// A Servarr config carrying a readable key, so the target opens.
const CONFIG: &str = "<Config><ApiKey>a1b2c3d4e5</ApiKey></Config>";

/// Where this test's records live, in a scratch directory of its own.
fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("lemonfiber-retract-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn paths(root: &Path) -> Paths {
    Paths::rooted(&root.join("config"), &root.join("data"))
}

/// A context over the real stack, answering services from the given transport.
fn ctx(root: &Path, http: Arc<Fake>) -> Ctx {
    Ctx::new(
        Arc::new(lemonfiber_core::adapters::Local),
        Arc::new(lemonfiber_core::adapters::Daemon::local()),
        Arc::new(lemonfiber_core::adapters::System),
        Files::ending(vec![("config/sonarr/config.xml", CONFIG)]),
        Source::External(project()),
        Settings {
            env_file: Some(paths(root).env_file()),
            stack_dir: Some(project().to_path_buf()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(http)
}

/// A repair that changed one field inside Sonarr.
fn configured() -> Change {
    Change {
        at: "2000".to_owned(),
        operation: OPERATION.to_owned(),
        target: "sonarr".to_owned(),
        kind: Kind::Configured {
            resource: "downloadclient".to_owned(),
            id: "7".to_owned(),
            field: "tvCategory".to_owned(),
            previous: Some("mine".to_owned()),
            current: "tv-sonarr".to_owned(),
        },
    }
}

/// Write a journal holding these changes where a reversal will read it.
fn journalled(root: &Path, changes: &[Change]) {
    let path = paths(root).journal();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let lines: Vec<String> = changes
        .iter()
        .map(|change| serde_json::to_string(change).unwrap_or_default())
        .collect();
    let _ = std::fs::write(path, lines.join("\n"));
}

/// The client as the service holds it, and its answer to being written back.
fn answering() -> Arc<Fake> {
    Fake::by_path(vec![(
        "downloadclient/7",
        Answer::reply(
            200,
            r#"{"id":7,"fields":[{"name":"host","value":"sabnzbd"},{"name":"tvCategory","value":"tv-sonarr"}]}"#,
        ),
    )])
}

/// The whole errand: a repair that changed a field inside a service is put back through
/// that service, with the operator's own value restored.
#[tokio::test]
async fn a_field_a_repair_changed_inside_a_service_goes_back_through_it() {
    let root = scratch("through-service");
    journalled(&root, &[configured()]);
    let http = answering();

    let put_back = retract(&ctx(&root, Arc::clone(&http)), &paths(&root)).await;

    assert!(put_back.is_ok_and(|undos| undos.len() == 1));
    let written = http
        .requests()
        .into_iter()
        .filter_map(|request| request.body)
        .collect::<Vec<_>>()
        .join(" ");
    assert!(written.contains("mine"), "{written}");
}

/// A service that will not answer leaves its change standing, and the operator is told
/// which one — rather than a reversal reporting success for work it could not do.
#[tokio::test]
async fn a_change_whose_service_is_gone_is_named_rather_than_reported_undone() {
    let root = scratch("service-gone");
    journalled(&root, &[configured()]);

    let put_back = retract(&ctx(&root, Fake::silent()), &paths(&root)).await;

    let problem = put_back
        .err()
        .expect("a change nobody could put back is a problem");
    assert!(
        problem
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("sonarr")),
        "{problem:?}"
    );
}

/// Nothing repaired is nothing to put back, and that is not a failure.
#[tokio::test]
async fn nothing_repaired_puts_nothing_back() {
    let root = scratch("nothing");
    journalled(&root, &[]);

    let put_back = retract(&ctx(&root, Fake::silent()), &paths(&root)).await;

    assert!(put_back.is_ok_and(|undos| undos.is_empty()));
}
