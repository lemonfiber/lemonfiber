use std::path::{Path, PathBuf};

use lemonfiber_fixtures::proving::Proving;
use lemonfiber_fixtures::scratch::Scratch;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::serving::Serves;

use super::super::program::tests::RUNNING;
use super::{record, Recorder, Took};
use crate::ports::program::{Program, Unran};

pub(super) mod asking;

const PINNED: &str = "ghcr.io/recyclarr/recyclarr@sha256:9c6c74d52cfc6f740a5a3624aa83867e171bb70b90453ddb7a4048b8c80445f9";
const FIXTURE: &str = "recordings/recyclarr/runs-version.json";

fn source(name: &str, manifest: &str) -> Scratch {
    let at = Scratch::new(name);
    let _ = std::fs::write(at.join("plugin.toml"), manifest);
    at
}

fn put(root: &Path, text: &str) {
    let at = root.join(FIXTURE);
    if let Some(parent) = at.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(at, text);
}

fn field(kept: &serde_json::Value, pointer: &str) -> serde_json::Value {
    kept.pointer(pointer).cloned().unwrap_or_default()
}

fn held(root: &Path) -> String {
    std::fs::read_to_string(root.join(FIXTURE)).unwrap_or_default()
}

fn took(read: &Result<super::Recorded, super::super::claimed::Unreadable>) -> Vec<Took> {
    read.as_ref()
        .map(|read| read.recordings.iter().map(|one| one.took.clone()).collect())
        .unwrap_or_default()
}

/// What a run-proved plugin is recorded with: `program`, and no engine to serve a
/// service or transport to ask one.
async fn ran(
    at: &Path,
    program: &dyn Program,
) -> Result<super::Recorded, super::super::claimed::Unreadable> {
    let serving = Serves::refusing(Unran::Unreachable {
        reason: "no engine in a test".to_owned(),
    });
    let silent =
        || -> std::sync::Arc<dyn crate::ports::http::Http> { Fake::always(Answer::Silent) };
    record(
        at,
        Recorder {
            program,
            serving: &serving,
            transport: &silent,
            random: &lemonfiber_fixtures::support::FixedRandom(None),
        },
    )
    .await
}

fn recording(from: &str, note: &str, args: &str, exit: i64, stdout: &str) -> String {
    format!(
        "{{\"recorded_from\": \"{from}\", \"note\": \"{note}\", \"run\": {{\"args\": {args}}}, \
         \"result\": {{\"exit\": {exit}, \"stdout_starts_with\": \"{stdout}\"}}}}"
    )
}

#[tokio::test]
async fn a_run_is_recorded_with_the_pin_its_arguments_and_the_first_kibibyte_it_wrote() {
    let at = source("record-written", RUNNING);
    let said = format!("v8.7.3 (Branch.master)\n{}", "x".repeat(2000));
    let program = Proving::exiting(0, &said);
    let read = ran(&at, program.as_ref()).await;
    assert_eq!(took(&read), vec![Took::Unnoted]);
    assert!(
        read.is_ok_and(|read| !read.holds),
        "a recording with no note fails the run"
    );
    let kept: serde_json::Value = serde_json::from_str(&held(&at)).unwrap_or_default();
    assert_eq!(field(&kept, "/recorded_from"), PINNED);
    assert_eq!(field(&kept, "/note"), "");
    assert_eq!(field(&kept, "/run/args"), serde_json::json!(["--version"]));
    assert_eq!(field(&kept, "/result/exit"), 0);
    assert_eq!(
        field(&kept, "/result/stdout_starts_with")
            .as_str()
            .map(str::len),
        Some(1024)
    );
    assert!(kept.get("request").is_none());
    let asked = program.asked();
    assert_eq!(asked.len(), 1);
    assert!(asked
        .first()
        .is_some_and(|one| one.env.contains(&"TZ=Europe/Amsterdam".to_owned())
            && one.user.as_deref() == Some("1000:1000")));
}

#[tokio::test]
async fn the_authors_note_is_kept_and_a_recording_of_another_pin_is_replaced() {
    let at = source("record-note", RUNNING);
    put(
        &at,
        &recording(
            "ghcr.io/recyclarr/recyclarr@sha256:old",
            "Why it matters.",
            "[\"--version\"]",
            0,
            "v8.7.1",
        ),
    );
    let read = ran(&at, Proving::exiting(0, "v8.7.3 (").as_ref()).await;
    assert_eq!(took(&read), vec![Took::Written]);
    assert!(read.is_ok_and(|read| read.holds));
    let kept: serde_json::Value = serde_json::from_str(&held(&at)).unwrap_or_default();
    assert_eq!(field(&kept, "/note"), "Why it matters.");
    assert_eq!(field(&kept, "/recorded_from"), PINNED);
    assert_eq!(field(&kept, "/result/stdout_starts_with"), "v8.7.3 (");
}

#[tokio::test]
async fn a_recording_the_run_reproduces_is_left_as_it_was() {
    let at = source("record-reproduced", RUNNING);
    let before = recording(PINNED, "n", "[\"--version\"]", 0, "v8.7.3 (");
    put(&at, &before);
    let read = ran(&at, Proving::exiting(0, "v8.7.3 (").as_ref()).await;
    assert_eq!(took(&read), vec![Took::Reproduced]);
    assert_eq!(held(&at), before);
}

#[tokio::test]
async fn a_recording_the_run_no_longer_reproduces_is_refused_naming_it_and_kept() {
    let at = source("record-unreproduced", RUNNING);
    let before = recording(PINNED, "n", "[\"--version\"]", 0, "v8.7.3 (a)");
    put(&at, &before);
    for (exit, stdout) in [(0, "v8.7.3 (b)"), (1, "v8.7.3 (a)")] {
        let read = ran(&at, Proving::exiting(exit, stdout).as_ref()).await;
        assert!(
            matches!(took(&read).first(), Some(Took::Unreproduced { why }) if why.contains("evidence of nothing")),
            "{exit} {stdout}: {read:?}"
        );
        assert!(read.is_ok_and(|read| !read.holds));
        assert_eq!(held(&at), before);
    }
}

#[tokio::test]
async fn an_unreproduced_recording_is_named_with_nothing_a_terminal_obeys() {
    let at = source("record-unreproduced-plain", RUNNING);
    put(
        &at,
        &recording(PINNED, "n", "[\"--version\"]", 0, "v8\\u001b[2Ja"),
    );
    let read = ran(&at, Proving::exiting(0, "v8\u{1b}[2Jb").as_ref()).await;
    assert!(
        matches!(took(&read).first(), Some(Took::Unreproduced { why })
            if why.contains("\"v8[2Ja\"") && why.contains("\"v8[2Jb\"") && !why.contains('\u{1b}')),
        "{read:?}"
    );
}

#[tokio::test]
async fn a_recording_of_other_arguments_is_replaced_rather_than_refused() {
    let at = source("record-other-args", RUNNING);
    put(&at, &recording(PINNED, "n", "[\"--help\"]", 0, "usage"));
    let read = ran(&at, Proving::exiting(0, "v8.7.3 (").as_ref()).await;
    assert_eq!(took(&read), vec![Took::Written]);
    let unnoted = source("record-reproduced-unnoted", RUNNING);
    put(
        &unnoted,
        &recording(PINNED, "", "[\"--version\"]", 0, "v8.7.3 ("),
    );
    let read = ran(&unnoted, Proving::exiting(0, "v8.7.3 (").as_ref()).await;
    assert_eq!(took(&read), vec![Took::Unnoted]);
}

#[tokio::test]
async fn a_run_that_overran_or_could_not_be_made_writes_nothing() {
    let at = source("record-unmade", RUNNING);
    let overran = ran(&at, Proving::overrunning("v").as_ref()).await;
    assert_eq!(took(&overran), vec![Took::Overran { seconds: 45 }]);
    let unmade = ran(
        &at,
        Proving::answering(Err(Unran::NoImage {
            image: PINNED.to_owned(),
        }))
        .as_ref(),
    )
    .await;
    assert!(
        matches!(took(&unmade).first(), Some(Took::Unrecorded { why }) if why.contains("pulls no image"))
    );
    assert!(unmade.is_ok_and(|read| !read.holds));
    assert_eq!(held(&at), "");
}

#[tokio::test]
async fn a_proof_naming_no_fixture_or_one_outside_the_source_is_not_recorded() {
    let none = source(
        "record-no-fixture",
        &RUNNING.replace("fixture = \"recordings/recyclarr/runs-version.json\"\n", ""),
    );
    let read = ran(&none, Proving::exiting(0, "v").as_ref()).await;
    assert!(
        matches!(took(&read).first(), Some(Took::Unrecorded { why }) if why.contains("names no fixture"))
    );
    let outside = source(
        "record-outside",
        &RUNNING.replace(FIXTURE, "../escaped.json"),
    );
    let read = ran(&outside, Proving::exiting(0, "v").as_ref()).await;
    assert!(read.as_ref().is_ok_and(|read| !read.holds), "{read:?}");
    assert!(!outside
        .path()
        .parent()
        .map(|up| up.join("escaped.json"))
        .is_some_and(|at: PathBuf| at.exists()));
}

#[cfg(unix)]
#[tokio::test]
async fn a_recording_is_never_written_through_a_link() {
    let elsewhere = Scratch::new("record-elsewhere");
    let at = source("record-linked", RUNNING);
    let _ = std::os::unix::fs::symlink(elsewhere.path(), at.join("recordings"));
    let read = ran(&at, Proving::exiting(0, "v8.7.3 (").as_ref()).await;
    assert!(
        matches!(took(&read).first(), Some(Took::Unrecorded { why }) if why.contains("is a link"))
    );
    assert!(!elsewhere.join("recyclarr/runs-version.json").exists());
}

#[tokio::test]
async fn a_refused_manifest_runs_nothing() {
    let at = source(
        "record-refused",
        &RUNNING.replace(
            "expect  = { exit = 0, stdout_starts_with = \"v8.7.3 (\" }",
            "expect  = { exit = 0 }",
        ),
    );
    let program = Proving::exiting(0, "v");
    let read = ran(&at, program.as_ref()).await;
    assert!(read
        .is_ok_and(|read| !read.refusals.is_empty() && read.recordings.is_empty() && !read.holds));
    assert!(program.asked().is_empty());
}

#[tokio::test]
async fn a_recording_that_cannot_be_written_is_unrecorded_naming_where() {
    let at = source("record-unwritable", RUNNING);
    let _ = std::fs::create_dir_all(at.join("recordings"));
    let _ = std::fs::write(
        at.join("recordings/recyclarr"),
        "a file where a directory goes",
    );
    let read = ran(&at, Proving::exiting(0, "v8.7.3 (").as_ref()).await;
    assert!(
        matches!(took(&read).first(), Some(Took::Unrecorded { why }) if why.contains("could not be written")),
        "{read:?}"
    );
}

#[tokio::test]
async fn a_run_proof_whose_recording_holds_a_request_is_run_and_its_note_kept() {
    let at = source("record-over-a-request", RUNNING);
    put(
        &at,
        "{\"recorded_from\": \"x@sha256:1\", \"note\": \"Kept.\", \"request\": {\"method\": \"GET\", \"path\": \"/x\"}, \"response\": {\"status\": 401}}",
    );
    let read = ran(&at, Proving::exiting(0, "v8.7.3 (").as_ref()).await;
    assert_eq!(took(&read), vec![Took::Written]);
    let kept: serde_json::Value = serde_json::from_str(&held(&at)).unwrap_or_default();
    assert_eq!(field(&kept, "/note"), "Kept.");
    assert_eq!(field(&kept, "/run/args"), serde_json::json!(["--version"]));
}

/// A claim's probe that runs is recorded by running it, after the run proofs and under
/// the name of the claim and the probe, from the service that provides the capability.
#[tokio::test]
async fn a_claim_probe_that_runs_is_recorded_by_running_it() {
    let manifest = format!(
        "{}\n[[claim]]\ncapability = \"quality.sync\"\n\n[[claim.probe]]\nid      = \"runs\"\n\
         run     = {{ args = [\"--version\"] }}\nexpect  = {{ exit = 0, stdout_starts_with = \"v8.\" }}\n\
         fixture = \"recordings/recyclarr/runs.json\"\n",
        RUNNING.replace(
            "posture       = \"confined\"",
            "posture       = \"confined\"\nprovides      = [\"quality.sync\"]"
        )
    );
    let read = lemonfiber_plugin::Manifest::from_toml(&manifest);
    assert!(read.is_ok(), "the fixture reads: {read:?}");
    let Ok(read) = read else { return };
    let runs = super::ran::runs(&read);
    let named: Vec<(&str, Option<&str>, Option<&str>)> = runs
        .iter()
        .map(|one| (one.named.as_str(), one.service, one.fixture))
        .collect();
    assert_eq!(
        named,
        vec![
            ("proof recyclarr.runs", None, Some(FIXTURE)),
            (
                "claim quality.sync probe runs",
                Some("qualitysync"),
                Some("recordings/recyclarr/runs.json")
            ),
        ]
    );
    let at = source("record-claim-run", &manifest);
    let program = Proving::exiting(0, "v8.7.3 (");
    let mut took = Vec::new();
    for running in runs {
        took.push(super::ran::taken(&read, running, &at, program.as_ref()).await);
    }
    let kept: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(at.join("recordings/recyclarr/runs.json")).unwrap_or_default(),
    )
    .unwrap_or_default();
    assert_eq!(
        took.iter().map(|one| one.took.clone()).collect::<Vec<_>>(),
        vec![Took::Unnoted, Took::Unnoted]
    );
    assert_eq!(field(&kept, "/recorded_from"), PINNED);
    assert_eq!(field(&kept, "/run/args"), serde_json::json!(["--version"]));
    assert_eq!(field(&kept, "/result/stdout_starts_with"), "v8.7.3 (");
}

/// A stream guard is recorded holding a throwaway certificate and its key where the core
/// writes them for it; any other service holds none.
#[test]
fn a_stream_guard_is_recorded_holding_a_certificate_the_core_would_write() {
    for (provides, holds) in [("stream.guard", true), ("media.serve", false)] {
        let manifest = RUNNING.replace(
            "posture       = \"confined\"",
            &format!("posture       = \"confined\"\nprovides      = [\"{provides}\"]"),
        );
        let placed = lemonfiber_plugin::Manifest::from_toml(&manifest)
            .ok()
            .and_then(|read| {
                crate::plugin::installed::Installed::of(&read)
                    .services
                    .into_iter()
                    .next()
            });
        let copy = crate::plugin::program::Throwaway::of(None);
        let (Some(placed), Ok(copy)) = (placed, copy) else {
            unreachable!("the fixture places a service and a copy is made");
        };
        assert!(super::served::certified(&placed, &copy).is_ok());
        let held = crate::certificate::kept(copy.path()).is_ok_and(|kept| kept.is_some() == holds);
        assert!(held, "{provides}");
    }
}

/// A guard whose certificate cannot be written into its copy is not recorded, naming why.
#[cfg(unix)]
#[test]
fn a_certificate_that_cannot_be_written_stops_the_guard_before_it_starts() {
    use std::os::unix::fs::PermissionsExt as _;
    let manifest = RUNNING.replace(
        "posture       = \"confined\"",
        "posture       = \"confined\"\nprovides      = [\"stream.guard\"]",
    );
    let placed = lemonfiber_plugin::Manifest::from_toml(&manifest)
        .ok()
        .and_then(|read| {
            crate::plugin::installed::Installed::of(&read)
                .services
                .into_iter()
                .next()
        });
    let copy = crate::plugin::program::Throwaway::of(None);
    let (Some(placed), Ok(copy)) = (placed, copy) else {
        unreachable!("the fixture places a service and a copy is made");
    };
    let _ = std::fs::set_permissions(copy.path(), std::fs::Permissions::from_mode(0o500));
    let certified = super::served::certified(&placed, &copy);
    let _ = std::fs::set_permissions(copy.path(), std::fs::Permissions::from_mode(0o700));
    assert!(certified.is_err());
}
