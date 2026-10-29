use std::path::Path;
use std::sync::Arc;

use lemonfiber_fixtures::proving::Proving;
use lemonfiber_fixtures::serving::Serves;
use lemonfiber_fixtures::support::FixedRandom;

use super::super::super::program::tests::RUNNING;
use super::super::{record, Failed, Recorded, Recorder, Retaking, Took};
use super::asking::{answering, source, SERVED};
use crate::ports::http::Http;
use crate::ports::program::Program;

const BUILT: &str = "first-party-live-built";

const PINNED: &str = "ghcr.io/recyclarr/recyclarr@sha256:9c6c74d52cfc6f740a5a3624aa83867e171bb70b90453ddb7a4048b8c80445f9";

/// [`RUNNING`] with its one service given the plugin's id.
fn owned() -> String {
    RUNNING.replace("\"qualitysync\"", "\"quality\"")
}

async fn built(at: &Path, program: &dyn Program, serving: &Serves) -> Option<Recorded> {
    let http = answering(r#"{"records": [], "totalRecords": 0}"#);
    let shared = || -> Arc<dyn Http> { Arc::clone(&http) as Arc<dyn Http> };
    record(
        at,
        Recorder {
            program,
            serving,
            transport: &shared,
            random: &FixedRandom(Some(vec![7; 32])),
            taking: &Retaking {
                built: Some(BUILT.to_owned()),
                ..Retaking::default()
            },
            confined: &lemonfiber_adapters::Disk,
            runner: &lemonfiber_fixtures::ports::Idle,
        },
    )
    .await
    .ok()
}

fn refused(read: Option<&Recorded>) -> Option<String> {
    read.and_then(|read| read.failed.first())
        .filter(|failed| {
            failed.step == "--built" && read.is_some_and(|read| read.recordings.is_empty())
        })
        .map(|Failed { why, .. }| why.clone())
}

#[tokio::test]
async fn a_run_of_the_plugins_own_service_is_performed_from_the_built_image_and_recorded_from_its_pin(
) {
    let at = source("record-built-run", &owned());
    let program = Proving::exiting(0, "v8.7.3");
    let read = built(&at, program.as_ref(), &Serves::on(49153)).await;
    assert_eq!(
        read.as_ref()
            .map(|read| read.recordings.iter().map(|one| one.took.clone()).collect()),
        Some(vec![Took::Unnoted])
    );
    let asked = program.asked();
    assert_eq!(
        asked
            .iter()
            .map(|one| one.image.as_str())
            .collect::<Vec<_>>(),
        vec![BUILT]
    );
    let kept: serde_json::Value =
        std::fs::read_to_string(at.join("recordings/recyclarr/runs-version.json"))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
    assert_eq!(
        kept.pointer("/recorded_from")
            .and_then(|from| from.as_str()),
        Some(PINNED)
    );
}

#[tokio::test]
async fn the_plugins_own_service_is_served_from_the_built_image() {
    let at = source("record-built-served", SERVED);
    let serving = Serves::on(49153);
    let program = Proving::unreachable();
    let read = built(&at, program.as_ref(), &serving).await;
    assert!(read.is_some_and(|read| read.failed.is_empty()));
    assert_eq!(
        serving
            .asked()
            .iter()
            .map(|one| one.image.as_str())
            .collect::<Vec<_>>(),
        vec![BUILT]
    );
}

#[test]
fn only_the_service_with_the_plugins_id_is_started_from_the_built_image() {
    let taking = Retaking {
        built: Some(BUILT.to_owned()),
        ..Retaking::default()
    };
    let placed = super::super::super::program::tests::placed(RUNNING);
    assert!(placed
        .as_ref()
        .is_some_and(|placed| taking.built_for("qualitysync", placed) == Some(BUILT)));
    assert!(placed
        .as_ref()
        .is_some_and(|placed| taking.built_for("quality", placed).is_none()));
    assert!(placed.is_some_and(|placed| Retaking::default()
        .built_for("qualitysync", &placed)
        .is_none()));
}

#[tokio::test]
async fn built_is_refused_where_no_service_has_the_plugins_id() {
    let at = source("record-built-none", RUNNING);
    let program = Proving::exiting(0, "v8.7.3");
    let read = built(&at, program.as_ref(), &Serves::on(49153)).await;
    assert!(refused(read.as_ref())
        .is_some_and(|why| why.contains("no service of this plugin has the plugin's id")));
    assert!(program.asked().is_empty());
}

#[tokio::test]
async fn built_is_refused_for_an_adapter() {
    let speaking = owned().replace(
        "user_mode     = \"uid\"",
        "user_mode     = \"uid\"\nspeaks        = [\"library.curate@1\"]",
    );
    let at = source("record-built-adapter", &speaking);
    let read = built(&at, Proving::exiting(0, "").as_ref(), &Serves::on(49153)).await;
    assert!(refused(read.as_ref()).is_some_and(|why| why.contains("is an adapter")));
}

#[tokio::test]
async fn built_is_refused_for_a_service_an_adapter_fronts() {
    let at = source(
        "record-built-fronted",
        super::super::fronted::tests::FRONTED,
    );
    let read = built(&at, Proving::exiting(0, "").as_ref(), &Serves::on(49153)).await;
    assert!(refused(read.as_ref()).is_some_and(|why| why.contains("an adapter fronts")));
}
