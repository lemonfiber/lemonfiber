//! Putting back one run of changes an operator picked out of the history.
//!
//! `doctor --undo` reverses the last repair and nothing else, and `rewind` unwinds the
//! whole journal. Between them sat the thing an operator actually asks for: *that* seed,
//! *that* reconfigure, the one they can see in `lemonfiber history` and now regret. This
//! drives naming a run by the stamp the history shows and getting exactly it back.
//!
//! From here rather than a `#[cfg(test)]` module for the reason the other app-layer paths
//! are: the crate is compiled twice, and a path exercised only in-crate has its coverage
//! counted from the copy that never ran.

mod common;

use common::stack::project;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_core::app::repair::{Left, Reversal};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::Settings;
use lemonfiber_core::journal::{Change, Kind};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::seams::Seams;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::{Answer, Fake};

/// The operation a first-run wizard records its work under.
const SEED: &str = "seed";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-onerun-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn paths(root: &Path) -> Paths {
    Paths::rooted(&root.join("config"), &root.join("data"))
}

/// A Servarr config carrying a readable key, so the target opens.
const CONFIG: &str = "<Config><ApiKey>a1b2c3d4e5</ApiKey></Config>";

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

/// A context whose Sonarr opens and answers, so a change inside it can go back.
fn reaching(root: &Path) -> Ctx {
    Ctx::new(
        Arc::new(lemonfiber_adapters::Local),
        Arc::new(lemonfiber_adapters::Daemon::local()),
        Arc::new(lemonfiber_adapters::System),
        Seams {
            filesystem: Files::ending(vec![("config/sonarr/config.xml", CONFIG)]),
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings {
            env_file: Some(paths(root).env_file()),
            stack_dir: Some(project().to_path_buf()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(answering())
}

fn ctx(root: &Path) -> Ctx {
    Ctx::new(
        Arc::new(lemonfiber_adapters::Local),
        Arc::new(lemonfiber_adapters::Daemon::local()),
        Arc::new(lemonfiber_adapters::System),
        Seams {
            filesystem: Files::ending(vec![]),
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings {
            env_file: Some(paths(root).env_file()),
            stack_dir: Some(project().to_path_buf()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(Fake::silent())
}

/// A setting one run of an operation wrote into lemonfiber's own environment file.
fn set(at: &str, operation: &str, key: &str, previous: Option<&str>, current: &str) -> Change {
    Change {
        at: at.to_owned(),
        operation: operation.to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: previous.map(str::to_owned),
            current: current.to_owned(),
        },
    }
}

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

fn env_holds(root: &Path, lines: &str) {
    let path = paths(root).env_file();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, lines);
}

fn reading(root: &Path) -> String {
    std::fs::read_to_string(paths(root).env_file()).unwrap_or_default()
}

/// Ask for a run back, answering with what the reversal came to.
async fn put_back(root: &Path, at: &str) -> Reversal {
    dispatch(
        Command::Undo {
            run: Some(at.to_owned()),
        },
        &ctx(root),
    )
    .await
    .ok()
    .and_then(|outcome| match outcome {
        Outcome::Undo(reversal) => Some(reversal),
        _ => None,
    })
    .unwrap_or_default()
}

/// Ask for a run back that cannot be, answering with the refusal as it reads.
async fn refusal(root: &Path, at: &str) -> String {
    dispatch(
        Command::Undo {
            run: Some(at.to_owned()),
        },
        &ctx(root),
    )
    .await
    .err()
    .map(|problem| format!("{problem:?}"))
    .unwrap_or_default()
}

/// The whole of one run goes back, and a run stamped differently is left alone.
#[tokio::test]
async fn one_run_goes_back_as_a_unit_and_the_others_stand() {
    let root = scratch("as-a-unit");
    env_holds(&root, "ONE=new\nTWO=new\nLATER=kept\n");
    journalled(
        &root,
        &[
            set("1000", SEED, "ONE", Some("old"), "new"),
            set("1000", SEED, "TWO", None, "new"),
            set("2000", "reconfigure", "LATER", Some("was"), "kept"),
        ],
    );

    let reversal = put_back(&root, "1000").await;
    assert_eq!(reversal.reversed.len(), 2, "{reversal:?}");
    assert!(reversal.left.is_empty(), "{reversal:?}");

    let env = reading(&root);
    assert!(env.contains("ONE=old"), "the value went back: {env}");
    assert!(
        !env.contains("TWO="),
        "a setting with nothing before is removed: {env}"
    );
    assert!(
        env.contains("LATER=kept"),
        "another run is untouched: {env}"
    );
}

/// A stamp naming no run is refused, and says so rather than reporting nothing done.
#[tokio::test]
async fn a_stamp_naming_no_run_is_refused_explicitly() {
    let root = scratch("no-run");
    env_holds(&root, "ONE=new\n");
    journalled(&root, &[set("1000", SEED, "ONE", Some("old"), "new")]);

    let said = refusal(&root, "9999").await;
    assert!(
        said.contains("9999"),
        "the refusal names the target: {said}"
    );
}

/// Putting a run back is itself journalled, so it can be put back in turn.
#[tokio::test]
async fn a_reversal_is_journalled_and_can_itself_be_put_back() {
    let root = scratch("rollable");
    env_holds(&root, "ONE=new\n");
    journalled(&root, &[set("1000", SEED, "ONE", Some("old"), "new")]);

    let reversal = put_back(&root, "1000").await;
    assert_eq!(reversal.reversed.len(), 1, "{reversal:?}");
    assert!(reading(&root).contains("ONE=old"));

    let journal = std::fs::read_to_string(paths(&root).journal()).unwrap_or_default();
    assert!(
        journal.contains("\"operation\":\"undo\""),
        "the reversal records itself: {journal}"
    );
}

/// A change somebody has edited since takes the whole run out of reach, and says why.
///
/// The judgement runs over every change before anything is touched: a reversal that
/// carried out three of five and then met one it could not is a machine in a state
/// nobody has been told about.
#[tokio::test]
async fn a_run_holding_an_edited_setting_is_not_offered() {
    let root = scratch("drifted");
    env_holds(&root, "ONE=new\nTWO=mine\n");
    journalled(
        &root,
        &[
            set("1000", SEED, "ONE", Some("old"), "new"),
            set("1000", SEED, "TWO", Some("was"), "new"),
        ],
    );

    let said = refusal(&root, "1000").await;
    assert!(said.contains("TWO"), "the refusal names the change: {said}");
    assert!(
        said.contains("discard"),
        "and says the edit would be discarded: {said}"
    );
    assert!(
        reading(&root).contains("ONE=new"),
        "and nothing was put back"
    );
}

/// A stamp two runs share is refused rather than guessed at.
#[tokio::test]
async fn a_stamp_naming_two_runs_is_refused() {
    let root = scratch("ambiguous");
    env_holds(&root, "ONE=new\nTWO=new\n");
    journalled(
        &root,
        &[
            set("1000", SEED, "ONE", Some("old"), "new"),
            set("1000", "reconfigure", "TWO", Some("old"), "new"),
        ],
    );

    let said = refusal(&root, "1000").await;
    assert!(said.contains("reconfigure"), "{said}");
    assert!(said.contains("seed"), "{said}");
    assert!(reading(&root).contains("ONE=new"), "nothing was put back");
}

/// A change only a service can undo, where that service will not answer, is reported —
/// alongside everything that did go back, rather than instead of it.
#[tokio::test]
async fn what_a_reversal_could_not_reach_is_reported_beside_what_it_did() {
    let root = scratch("partial");
    env_holds(&root, "ONE=new\n");
    journalled(
        &root,
        &[
            set("1000", SEED, "ONE", Some("old"), "new"),
            Change {
                at: "1000".to_owned(),
                operation: SEED.to_owned(),
                target: "sonarr".to_owned(),
                kind: Kind::Configured {
                    resource: "downloadclient".to_owned(),
                    id: "7".to_owned(),
                    field: "tvCategory".to_owned(),
                    previous: Some("mine".to_owned()),
                    current: "tv-sonarr".to_owned(),
                },
            },
        ],
    );

    let reversal = put_back(&root, "1000").await;
    assert_eq!(
        reversal.reversed.len(),
        1,
        "the setting went back: {reversal:?}"
    );
    assert_eq!(
        reversal.left.len(),
        1,
        "the service change did not: {reversal:?}"
    );
    // Compared whole rather than probed field by field: what the report says is the
    // sentence an operator reads, and "contains the word answer" would pass on a
    // report that had lost the target it is about.
    assert_eq!(
        reversal.left,
        vec![Left {
            target: "downloadclient in sonarr".to_owned(),
            because: "the service that made it did not answer".to_owned(),
        }]
    );
    assert!(
        reading(&root).contains("ONE=old"),
        "and the host half is done"
    );
}

/// A run that cannot say where lemonfiber keeps its record is refused, not guessed at.
#[tokio::test]
async fn a_machine_that_cannot_say_where_its_record_is_refuses() {
    let nowhere = Ctx::new(
        Arc::new(lemonfiber_adapters::Local),
        Arc::new(lemonfiber_adapters::Daemon::local()),
        Arc::new(lemonfiber_adapters::System),
        Seams {
            filesystem: Files::ending(vec![]),
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings::default(),
        Environment::MacOs,
    )
    .with_http(Fake::silent());

    let said = dispatch(
        Command::Undo {
            run: Some("1000".to_owned()),
        },
        &nowhere,
    )
    .await
    .err()
    .map(|problem| format!("{problem:?}"))
    .unwrap_or_default();

    assert!(said.contains("nowhere"), "{said}");
    assert!(
        said.contains("Nothing was put back"),
        "and says so plainly: {said}"
    );
}

/// A field inside a service goes back through that service, and the record of the
/// reversal says so — which is what lets the reversal itself be put back in turn.
#[tokio::test]
async fn a_field_inside_a_service_goes_back_and_the_record_says_so() {
    let root = scratch("reaching");
    env_holds(&root, "ONE=new\n");
    journalled(
        &root,
        &[
            set("1000", SEED, "ONE", Some("old"), "new"),
            Change {
                at: "1000".to_owned(),
                operation: SEED.to_owned(),
                target: "sonarr".to_owned(),
                kind: Kind::Configured {
                    resource: "downloadclient".to_owned(),
                    id: "7".to_owned(),
                    field: "tvCategory".to_owned(),
                    previous: Some("mine".to_owned()),
                    current: "tv-sonarr".to_owned(),
                },
            },
        ],
    );

    let reversal = dispatch(
        Command::Undo {
            run: Some("1000".to_owned()),
        },
        &reaching(&root),
    )
    .await
    .ok()
    .and_then(|outcome| match outcome {
        Outcome::Undo(reversal) => Some(reversal),
        _ => None,
    })
    .unwrap_or_default();

    assert_eq!(reversal.reversed.len(), 2, "both went back: {reversal:?}");
    assert!(reversal.left.is_empty(), "{reversal:?}");

    let journal = std::fs::read_to_string(paths(&root).journal()).unwrap_or_default();
    assert!(
        journal.contains("\"action\":\"configured\"") && journal.contains("\"operation\":\"undo\""),
        "the service-side reversal is recorded with its values the other way round: {journal}"
    );
}

/// A directory the run made comes off with it, and the record of the reversal does not
/// claim the directory was made again.
///
/// The journal can spell the inverse of a setting and of a field inside a service, and
/// of nothing else. A path that was removed has no inverse — writing one would put a
/// line in the record that replaying would not reproduce — so the reversal records the
/// settings it put back and stays silent about the directory it took away.
#[tokio::test]
async fn a_path_the_run_made_comes_off_and_is_not_recorded_as_made_again() {
    let root = scratch("made");
    env_holds(&root, "ONE=new\n");
    let made = root.join("made-by-the-run");
    let _ = std::fs::create_dir_all(&made);

    journalled(
        &root,
        &[
            set("1000", SEED, "ONE", Some("old"), "new"),
            Change {
                at: "1000".to_owned(),
                operation: SEED.to_owned(),
                target: made.to_string_lossy().into_owned(),
                kind: Kind::Made {
                    path: made.to_string_lossy().into_owned(),
                },
            },
        ],
    );

    let reversal = put_back(&root, "1000").await;

    assert_eq!(reversal.reversed.len(), 2, "both went back: {reversal:?}");
    assert!(!made.exists(), "the directory came off");

    // The original entries stay as they are; what is read here is only what the
    // reversal wrote about itself.
    let journal = std::fs::read_to_string(paths(&root).journal()).unwrap_or_default();
    let wrote: Vec<&str> = journal
        .lines()
        .filter(|line| line.contains("\"operation\":\"undo\""))
        .collect();

    assert_eq!(wrote.len(), 1, "one entry, for the setting: {journal}");
    assert!(
        wrote
            .first()
            .is_some_and(|line| line.contains("\"action\":\"set\"")),
        "and it is the setting rather than the directory: {wrote:?}"
    );
}

/// A machine whose stack cannot be read refuses, rather than putting back the half of a
/// run that needs no stack.
///
/// Which services exist is what says where a change that lives inside one goes back
/// through, so a reversal that could not read the stack does not know whether the run it
/// was handed needs a service at all. It stops before touching anything, which is the
/// same rule the judgement above it keeps.
#[tokio::test]
async fn a_stack_that_cannot_be_read_stops_the_reversal() {
    let root = scratch("no-stack");
    env_holds(&root, "ONE=new\n");
    journalled(&root, &[set("1000", SEED, "ONE", Some("old"), "new")]);

    let nowhere = Ctx::new(
        Arc::new(lemonfiber_adapters::Local),
        Arc::new(lemonfiber_adapters::Daemon::local()),
        Arc::new(lemonfiber_adapters::System),
        Seams {
            filesystem: Files::ending(vec![]),
            ..lemonfiber_adapters::live()
        },
        Source::External(Path::new("/lemonfiber/no/such/stack")),
        Settings {
            env_file: Some(paths(&root).env_file()),
            stack_dir: Some(project().to_path_buf()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(Fake::silent());

    let refused = dispatch(
        Command::Undo {
            run: Some("1000".to_owned()),
        },
        &nowhere,
    )
    .await
    .err()
    .is_some();

    assert!(refused, "a stack that cannot be read did not stop it");
    assert!(
        reading(&root).contains("ONE=new"),
        "and nothing was put back"
    );
}
