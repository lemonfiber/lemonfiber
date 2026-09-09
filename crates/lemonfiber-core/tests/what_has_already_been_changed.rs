//! The record of what lemonfiber changed, dispatched as the surfaces reach it.
//!
//! Beside the crate rather than inside it because the dispatcher is compiled twice —
//! once with the crate's own tests and once without — so an arm exercised only in-crate
//! has its coverage counted from the copy that never ran.
//!
//! What is asserted here is the half the pure judgement cannot: that the command routes,
//! that it reads the journal a real apply wrote rather than one handed to it, and that
//! the value a setting currently holds reaches the drift question. A history that read a
//! journal but never the environment file would report every setting reversible, which is
//! the one answer that costs somebody their own edit.

mod common;

use common::stack::project;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::Settings;
use lemonfiber_core::journal::{Change, Kind};
use lemonfiber_core::model::HistoryReport;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::files::Files;

/// Where this test's records live, in a scratch directory of its own.
fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("lemonfiber-history-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn paths(root: &Path) -> Paths {
    Paths::rooted(&root.join("config"), &root.join("data"))
}

/// A context over a real directory, so the journal and the environment file are read
/// from disk rather than answered by a fake.
fn ctx(root: &Path) -> Ctx {
    Ctx::new(
        Arc::new(lemonfiber_core::adapters::Local),
        Arc::new(lemonfiber_core::adapters::Daemon::local()),
        Arc::new(lemonfiber_core::adapters::System),
        Files::ending(vec![]),
        Source::External(project()),
        Settings {
            env_file: Some(paths(root).env_file()),
            stack_dir: Some(project().to_path_buf()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
}

/// One setting written by an operation.
fn set(operation: &str, key: &str, previous: Option<&str>, current: &str) -> Change {
    Change {
        at: "2000".to_owned(),
        operation: operation.to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: previous.map(str::to_owned),
            current: current.to_owned(),
        },
    }
}

/// Write a journal where the history will read it.
fn journalled(root: &Path, changes: &[Change]) {
    let path = paths(root).journal();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let lines: Vec<String> = changes
        .iter()
        .filter_map(|change| serde_json::to_string(change).ok())
        .collect();
    let _ = std::fs::write(path, lines.join("\n"));
}

/// Write an environment file holding these values, as an operator's own edit would.
fn holding(root: &Path, pairs: &[(&str, &str)]) {
    let path = paths(root).env_file();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let body: Vec<String> = pairs
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    let _ = std::fs::write(path, body.join("\n"));
}

/// The record, or nothing where the command answered with something else.
async fn recorded(ctx: &Ctx) -> Option<HistoryReport> {
    match dispatch(Command::History, ctx).await {
        Ok(Outcome::History(report)) => Some(report),
        _ => None,
    }
}

/// The record reads newest first, because the change somebody is asking about is nearly
/// always the one they just made.
#[tokio::test]
async fn the_record_reads_newest_first() {
    let root = scratch("order");
    journalled(
        &root,
        &[
            set("apply", "TZ", None, "Europe/Amsterdam"),
            set("reconfigure", "PUID", None, "1000"),
        ],
    );

    let report = recorded(&ctx(&root)).await;
    let changes = report.map(|report| report.changes).unwrap_or_default();

    assert_eq!(changes.len(), 2, "both changes were read");
    let first = changes.first().map(|change| change.did.clone());
    assert_eq!(first, Some("set PUID to 1000".to_owned()));
}

/// A setting somebody has edited since is refused rather than quietly overwritten, and
/// the refusal names what it holds now — which is only knowable by reading the file.
#[tokio::test]
async fn a_setting_edited_since_is_refused_and_says_what_it_holds() {
    let root = scratch("drift");
    journalled(&root, &[set("apply", "TZ", None, "Europe/Amsterdam")]);
    holding(&root, &[("TZ", "America/New_York")]);

    let report = recorded(&ctx(&root)).await;
    let changes = report.map(|report| report.changes).unwrap_or_default();
    let first = changes.first();

    assert_eq!(
        first.map(|change| change.reversal.clone()),
        Some("none".to_owned()),
        "an edit somebody made is not overwritten"
    );
    let because = first.and_then(|change| change.because.clone());
    assert!(
        because.is_some_and(|said| said.contains("America/New_York")),
        "the refusal names what the setting holds now"
    );
}

/// A setting the file still holds as this change left it is reversible, which is the
/// other half of the same read — a history that never opened the file would say this
/// about the edited one too.
#[tokio::test]
async fn a_setting_still_holding_what_it_was_left_can_go_back() {
    let root = scratch("undrifted");
    journalled(
        &root,
        &[set("apply", "TZ", Some("UTC"), "Europe/Amsterdam")],
    );
    holding(&root, &[("TZ", "Europe/Amsterdam")]);

    let report = recorded(&ctx(&root)).await;
    let changes = report.map(|report| report.changes).unwrap_or_default();

    assert_eq!(
        changes.first().map(|change| change.reversal.clone()),
        Some("whole".to_owned())
    );
}

/// What one operation took is said on each of its lines, because the operation is the
/// unit that goes back and half of one leaves a machine in a state nobody chose.
#[tokio::test]
async fn a_change_says_how_many_went_with_it() {
    let root = scratch("together");
    // An earlier seed of the same machine, under the same name and a different stamp.
    // Without it a count that gathered every seed ever made would read the same as one
    // that gathered this run's, and this test would pass on both.
    let earlier = |key: &str| Change {
        at: "1000".to_owned(),
        ..set("seed", key, None, "1000")
    };
    journalled(
        &root,
        &[
            earlier("OLD_PUID"),
            earlier("OLD_PGID"),
            set("seed", "PUID", None, "1000"),
            set("seed", "PGID", None, "1000"),
            set("reconfigure", "TZ", None, "UTC"),
        ],
    );

    let report = recorded(&ctx(&root)).await;
    let changes = report.map(|report| report.changes).unwrap_or_default();
    let alone = changes
        .iter()
        .find(|change| change.did.contains("TZ"))
        .map(|change| change.alongside);
    let seeded = changes
        .iter()
        .find(|change| change.did.contains(" PUID"))
        .map(|change| change.alongside);

    assert_eq!(changes.len(), 5, "every change is on the record");
    assert_eq!(alone, Some(1), "the lone reconfigure took nothing with it");
    assert_eq!(
        seeded,
        Some(2),
        "this seed wrote two, and the earlier seed is a run of its own"
    );
}

/// A machine that has changed nothing answers with an empty record rather than a
/// refusal, and still says how far back the record goes — an empty history and a
/// trimmed one are the same list otherwise.
#[tokio::test]
async fn a_machine_that_changed_nothing_still_states_its_horizon() {
    let root = scratch("empty");
    let report = recorded(&ctx(&root)).await;

    let horizon = report.as_ref().map(|report| report.horizon.clone());
    assert!(
        horizon.is_some_and(|said| !said.is_empty()),
        "the horizon is stated even with nothing on the record"
    );
    assert_eq!(report.map(|report| report.changes.len()), Some(0));
}

/// A resource one of the services now holds, made by an operation.
fn created(resource: &str, id: &str) -> Change {
    Change {
        at: "2000".to_owned(),
        operation: "seed".to_owned(),
        target: "sonarr".to_owned(),
        kind: Kind::Created {
            resource: resource.to_owned(),
            id: id.to_owned(),
        },
    }
}

/// A path lemonfiber itself made on this machine.
fn made(path: &str) -> Change {
    Change {
        at: "2000".to_owned(),
        operation: "apply".to_owned(),
        target: path.to_owned(),
        kind: Kind::Made {
            path: path.to_owned(),
        },
    }
}

/// One field of one resource a service holds, changed through that service.
fn configured(field: &str, previous: Option<&str>, current: &str) -> Change {
    Change {
        at: "2000".to_owned(),
        operation: "seed".to_owned(),
        target: "sonarr".to_owned(),
        kind: Kind::Configured {
            resource: "downloadclient".to_owned(),
            id: "3".to_owned(),
            field: field.to_owned(),
            previous: previous.map(str::to_owned),
            current: current.to_owned(),
        },
    }
}

/// Each kind of change is a different event and reads as one. A record that worded
/// them alike would tell an operator that a directory lemonfiber made and a download
/// client it registered were the same sort of thing, which is what they consult a
/// history to tell apart.
#[tokio::test]
async fn every_kind_of_change_reads_as_the_sentence_it_was() {
    let root = scratch("kinds");
    journalled(
        &root,
        &[
            created("downloadclient", "3"),
            made("/srv/media"),
            configured("removeCompletedDownloads", Some("false"), "true"),
            set("reconfigure", "PUID", Some("1000"), "1001"),
            set("apply", "TZ", None, "Europe/Amsterdam"),
        ],
    );

    let report = recorded(&ctx(&root)).await;
    let changes = report.map(|report| report.changes).unwrap_or_default();
    let said: Vec<String> = changes.iter().map(|change| change.did.clone()).collect();

    assert_eq!(
        said,
        [
            "set TZ to Europe/Amsterdam",
            "changed PUID from 1000 to 1001",
            "set removeCompletedDownloads on the service",
            "made /srv/media",
            "added a downloadclient",
        ],
        "newest first, and every kind in the words of what it did"
    );
}

/// A service's own record goes back through the service that owns it, so nothing about
/// it is a setting and the drift question never arises. Reversible in full, and the
/// record says so without a reason attached to it.
#[tokio::test]
async fn a_change_to_a_services_own_record_can_go_back_in_full() {
    let root = scratch("services");
    journalled(&root, &[created("downloadclient", "3"), made("/srv/media")]);

    let report = recorded(&ctx(&root)).await;
    let changes = report.map(|report| report.changes).unwrap_or_default();
    let verdicts: Vec<String> = changes
        .iter()
        .map(|change| change.reversal.clone())
        .collect();

    assert_eq!(verdicts, ["whole", "whole"]);
    assert!(
        changes.iter().all(|change| change.because.is_none()),
        "nothing stands in the way, so nothing is offered as a reason"
    );
}

/// The pointer to the library goes back and the library does not follow it. Said as
/// partly reversible with what the rest leaves, because an operator told this change
/// is reversible would expect their files to move back with the setting.
#[tokio::test]
async fn putting_the_data_location_back_is_only_partly_possible_and_says_what_stays() {
    let root = scratch("pointer");
    journalled(
        &root,
        &[set(
            "reconfigure",
            "DATA_ROOT",
            Some("/srv/old"),
            "/srv/new",
        )],
    );
    holding(&root, &[("DATA_ROOT", "/srv/new")]);

    let report = recorded(&ctx(&root)).await;
    let changes = report.map(|report| report.changes).unwrap_or_default();
    let first = changes.first();

    assert_eq!(
        first.map(|change| change.reversal.clone()),
        Some("partial".to_owned()),
        "neither reversible nor refused"
    );
    let because = first.and_then(|change| change.because.clone());
    assert!(
        because.is_some_and(|said| said.contains("the data does not move with it")),
        "the record says what putting it back would leave behind"
    );
    let instead = first.and_then(|change| change.instead.clone());
    assert!(instead.is_some_and(|said| said.contains("move the library yourself")));
}
