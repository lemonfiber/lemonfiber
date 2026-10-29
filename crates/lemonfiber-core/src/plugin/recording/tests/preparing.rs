use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_fixtures::proving::Proving;
use lemonfiber_fixtures::serving::Serves;
use lemonfiber_fixtures::support::FixedRandom;

use super::super::super::program::tests::RUNNING;
use super::super::{record, Recorded, Recorder, Retaking, Took};
use super::asking::{answering, source, SERVED};
use crate::plugin::program::{Preparing, Throwaway, PREPARE};
use crate::ports::http::Http;
use crate::ports::program::Program;

/// Where the script below notes each copy it was handed, beneath the plugin's root.
const NOTED: &str = "prepared-copies";

/// A `posture/prepare` that notes the copy it was handed and then runs `then`.
fn script(root: &Path, then: &str) {
    let at = root.join(PREPARE);
    let _ = at.parent().map(std::fs::create_dir_all);
    let _ = std::fs::write(
        &at,
        format!("#!/bin/sh\nset -eu\necho \"$1\" >> {NOTED}\n{then}\n"),
    );
    let _ = std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755));
}

/// Each copy the script was handed, in turn.
fn noted(root: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(root.join(NOTED))
        .unwrap_or_default()
        .lines()
        .map(PathBuf::from)
        .collect()
}

async fn recorded(at: &Path, program: &dyn Program, serving: &Serves) -> Option<Recorded> {
    let http = answering(r#"{"records": [], "totalRecords": 0}"#);
    let shared = || -> Arc<dyn Http> { Arc::clone(&http) as Arc<dyn Http> };
    record(
        at,
        Recorder {
            program,
            serving,
            transport: &shared,
            random: &FixedRandom(Some(vec![7; 32])),
            taking: &Retaking::default(),
            confined: &lemonfiber_adapters::Disk,
            runner: &lemonfiber_adapters::Local,
        },
    )
    .await
    .ok()
}

/// The first service that failed, at the step it failed at, and why.
fn failed(read: Option<&Recorded>) -> Option<(String, String)> {
    read.and_then(|read| read.failed.first())
        .map(|failed| (failed.step.clone(), failed.why.clone()))
}

#[tokio::test]
async fn the_plugins_own_service_is_served_with_the_copy_its_script_prepared() {
    let at = source("record-prepared-served", SERVED);
    script(&at, "exit 0");
    let serving = Serves::on(49153);
    let read = recorded(&at, Proving::unreachable().as_ref(), &serving).await;
    assert!(read.is_some_and(|read| read.failed.is_empty()));
    let handed = noted(&at);
    let served = serving.asked();
    assert_eq!(handed.len(), 1);
    assert_eq!(served.len(), 1);
    assert!(served
        .iter()
        .flat_map(|serve| &serve.binds)
        .any(|bound| handed.contains(&bound.source)));
}

#[tokio::test]
async fn a_service_whose_script_fails_is_not_started() {
    let at = source("record-prepared-refused", SERVED);
    script(&at, "exit 1");
    let serving = Serves::on(49153);
    let read = recorded(&at, Proving::unreachable().as_ref(), &serving).await;
    assert_eq!(
        failed(read.as_ref()),
        Some((PREPARE.to_owned(), "posture/prepare exited 1".to_owned()))
    );
    assert!(serving.asked().is_empty());
}

#[tokio::test]
async fn a_run_of_the_plugins_own_service_is_performed_with_the_copy_its_script_prepared() {
    let at = source(
        "record-prepared-run",
        &RUNNING.replace("\"qualitysync\"", "\"quality\""),
    );
    script(&at, "exit 0");
    let program = Proving::exiting(0, "v8.7.3");
    let read = recorded(&at, program.as_ref(), &Serves::on(49153)).await;
    assert_eq!(
        read.map(|read| read.recordings.into_iter().map(|one| one.took).collect()),
        Some(vec![Took::Unnoted])
    );
    let handed = noted(&at);
    assert_eq!(handed.len(), 1);
    assert_eq!(
        program
            .asked()
            .iter()
            .filter_map(|run| run.configuration.as_ref())
            .map(|configuration| configuration.source.clone())
            .collect::<Vec<_>>(),
        handed
    );
}

#[tokio::test]
async fn a_run_whose_script_fails_is_not_performed() {
    let at = source(
        "record-prepared-run-refused",
        &RUNNING.replace("\"qualitysync\"", "\"quality\""),
    );
    script(&at, "exit 1");
    let program = Proving::exiting(0, "v8.7.3");
    let read = recorded(&at, program.as_ref(), &Serves::on(49153)).await;
    assert_eq!(
        read.map(|read| read.recordings.into_iter().map(|one| one.took).collect()),
        Some(vec![Took::Unrecorded {
            why: "posture/prepare exited 1".to_owned()
        }])
    );
    assert!(program.asked().is_empty());
}

#[tokio::test]
async fn no_other_service_is_handed_to_the_script() {
    let at = source("record-prepared-other", RUNNING);
    script(&at, "exit 1");
    let program = Proving::exiting(0, "v8.7.3");
    let read = recorded(&at, program.as_ref(), &Serves::on(49153)).await;
    assert_eq!(
        read.map(|read| read.recordings.into_iter().map(|one| one.took).collect()),
        Some(vec![Took::Unnoted])
    );
    assert!(noted(&at).is_empty());
}

#[tokio::test]
async fn a_fronted_service_whose_script_fails_is_not_started() {
    let at = source(
        "record-prepared-fronted",
        super::super::fronted::tests::FRONTED,
    );
    script(&at, "exit 1");
    let serving = Serves::on(49153);
    let read = recorded(&at, Proving::unreachable().as_ref(), &serving).await;
    assert_eq!(
        failed(read.as_ref()),
        Some((PREPARE.to_owned(), "posture/prepare exited 1".to_owned()))
    );
    assert_eq!(noted(&at).len(), 1);
    assert!(serving.asked().is_empty());
}

#[tokio::test]
async fn what_the_recorder_writes_takes_the_place_of_what_the_script_left() {
    let at = source("record-prepared-certificate", RUNNING);
    script(&at, "echo 'not a certificate' > \"$1/certificate.pem\"");
    let guard = RUNNING.replace(
        "posture       = \"confined\"",
        "posture       = \"confined\"\nprovides      = [\"stream.guard\"]",
    );
    let placed = lemonfiber_plugin::Manifest::from_toml(&guard)
        .ok()
        .and_then(|read| {
            crate::plugin::installed::Installed::of(&read)
                .services
                .into_iter()
                .next()
        });
    let copy = Throwaway::of(None, &lemonfiber_fixtures::ports::Chance::cycling()).ok();
    let (Some(placed), Some(copy)) = (placed, copy) else {
        unreachable!("the fixture places a service and a copy is made");
    };
    let prepared = Preparing {
        runner: &lemonfiber_adapters::Local,
        root: &at,
    }
    .prepared(&copy)
    .await;
    assert_eq!(prepared, Ok(()));
    let certified = super::super::served::certified(&placed, &copy, &lemonfiber_adapters::Disk);
    let kept = crate::certificate::kept(&lemonfiber_adapters::Disk, copy.path())
        .ok()
        .flatten();
    assert!(kept.is_some());
    assert_eq!(certified, Ok(kept.map(|kept| kept.fingerprint)));
}
