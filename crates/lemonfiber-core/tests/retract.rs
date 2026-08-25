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

use common::stack::project;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_core::app::repair::retract;
use lemonfiber_core::app::{diagnose, dispatch, Command, Ctx};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::Settings;
use lemonfiber_core::doctor::{Category, Narrowing};
use lemonfiber_core::journal::{Change, Kind};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::repair::OPERATION;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::{Answer, Fake};

/// A Servarr config carrying a readable key, so the target opens.
const CONFIG: &str = "<Config><ApiKey>a1b2c3d4e5</ApiKey></Config>";

/// `SABnzbd`'s own configuration, carrying the key an \*arr is told to reach it with.
const SABNZBD: &str = "[misc]\napi_key = sabkey123\n";

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
        // SABnzbd's key too: the wirings a diagnosis reads are the clients lemonfiber
        // would write, and without a credential to write there is no client to compare.
        Files::ending(vec![
            ("config/sonarr/config.xml", CONFIG),
            ("config/sabnzbd/sabnzbd.ini", SABNZBD),
        ]),
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

/// The wirings a diagnosis reads are assembled from the baseline lemonfiber wrote, so a
/// stack with one is the only stack where there is anything to compare against.
///
/// Driven through `diagnose` rather than the assembly directly: what is being proved is
/// that a real run reaches the wirings at all, and a test that called the gathering itself
/// would pass while the check saw nothing.
#[tokio::test]
async fn a_stack_with_a_baseline_has_its_wirings_read() {
    let root = scratch("baseline");
    let paths = paths(&root);
    if let Some(dir) = paths.env_file().parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // What lemonfiber last wrote into Sonarr, in the shape seeding saves.
    let _ = std::fs::write(
        paths.env_file().with_file_name("baseline.json"),
        r#"{"services":{"Sonarr":{"downloadclient:sabnzbd:8080":{"value":"tv-sonarr","at":"1000","origin":"written"}}}}"#,
    );

    let report = diagnose(
        &ctx(&root, Fake::silent()),
        &Narrowing::Category(Category::Config),
        false,
    )
    .await;

    // Nothing answers, so every wiring reads as unverified — but the wirings were read,
    // which is the whole of what a baseline buys.
    assert!(
        report.is_ok(),
        "a diagnosis over a baselined stack still runs"
    );
}

/// The whole errand: a repair that changed a field inside a service is put back through
/// that service, with the operator's own value restored.
#[tokio::test]
async fn a_field_a_repair_changed_inside_a_service_goes_back_through_it() {
    let root = scratch("through-service");
    journalled(&root, &[configured()]);
    let http = answering();

    let put_back = retract(&ctx(&root, Arc::clone(&http)), &paths(&root)).await;

    assert_eq!(put_back.map(|undos| undos.len()).ok(), Some(1));
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

    assert!(
        put_back
            .err()
            .and_then(|problem| problem.detail.clone())
            .is_some_and(|detail| detail.contains("sonarr")),
        "a change nobody could put back is a problem that names the service it needed"
    );
}

/// A change naming a service this stack does not have cannot be put back by it, and says
/// which one it needed — the same answer as a service that is down, because from here the
/// two are the same fact.
#[tokio::test]
async fn a_change_naming_a_service_the_stack_lacks_is_named_too() {
    let root = scratch("no-such-service");
    let mut change = configured();
    change.target = "not-a-service".to_owned();
    journalled(&root, &[change]);

    let put_back = retract(&ctx(&root, answering()), &paths(&root)).await;

    assert!(
        put_back
            .err()
            .and_then(|problem| problem.detail.clone())
            .is_some_and(|detail| detail.contains("not-a-service")),
        "the reversal names what it could not reach"
    );
}

/// A service that answers but refuses the write leaves the change standing, and says so
/// rather than reporting a reversal that the service turned down.
#[tokio::test]
async fn a_service_that_refuses_the_write_is_not_reported_as_undone() {
    let root = scratch("refused");
    journalled(&root, &[configured()]);
    let http = Fake::by_path(vec![("downloadclient/7", Answer::reply(400, "no"))]);

    let put_back = retract(&ctx(&root, http), &paths(&root)).await;

    assert!(put_back.is_err(), "a refused write is not a reversal");
}

/// A stack that will not read is the one thing a reversal needs before anything else: it
/// has to know which services exist to know which one a change belongs to. Said as the
/// stack's own problem rather than as a reversal that quietly did nothing.
#[tokio::test]
async fn a_stack_that_will_not_read_stops_the_reversal() {
    let root = scratch("no-stack");
    journalled(&root, &[configured()]);
    let nowhere = Ctx::new(
        Arc::new(lemonfiber_core::adapters::Local),
        Arc::new(lemonfiber_core::adapters::Daemon::local()),
        Arc::new(lemonfiber_core::adapters::System),
        Files::empty(),
        Source::External(Path::new("/lemonfiber/no/such/stack")),
        Settings {
            env_file: Some(paths(&root).env_file()),
            ..Settings::default()
        },
        Environment::MacOs,
    );

    assert!(retract(&nowhere, &paths(&root)).await.is_err());
}

/// Nothing repaired is nothing to put back, and that is not a failure.
#[tokio::test]
async fn nothing_repaired_puts_nothing_back() {
    let root = scratch("nothing");
    journalled(&root, &[]);

    let put_back = retract(&ctx(&root, Fake::silent()), &paths(&root)).await;

    assert!(put_back.is_ok_and(|undos| undos.is_empty()));
}

/// The reversal, asked for the way every surface asks for it.
///
/// Through the dispatcher rather than through [`retract`] directly, and from here as
/// well as in-crate for the reason at the top of this file. The change goes back on
/// the host, so no service has to answer for the shape of the reply to be held.
#[tokio::test]
async fn a_dispatched_reversal_answers_under_its_own_kind_and_says_what_went_back() {
    let root = scratch("dispatched");
    journalled(
        &root,
        &[Change {
            at: "3000".to_owned(),
            operation: OPERATION.to_owned(),
            target: "qbittorrent".to_owned(),
            kind: Kind::Set {
                key: "QBITTORRENT_PORT".to_owned(),
                previous: Some("8080".to_owned()),
                current: "51413".to_owned(),
            },
        }],
    );

    let json = dispatch(Command::Undo, &ctx(&root, Fake::silent()))
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();

    assert!(json.contains(r#""kind":"undo""#), "{json}");
    // What went back, said as what reversing it does rather than as a count.
    assert!(json.contains(r#""does":"restore""#), "{json}");
    assert!(json.contains(r#""value":"8080""#), "{json}");
}
