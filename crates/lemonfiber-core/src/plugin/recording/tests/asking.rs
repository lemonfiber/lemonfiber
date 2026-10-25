use std::path::Path;
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::proving::Proving;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_fixtures::serving::Serves;
use lemonfiber_fixtures::support::FixedRandom;

use super::super::{record, Failed, Recorded, Recorder, Took};
use crate::ports::http::Http;
use crate::ports::program::Unran;
use crate::ports::serving::Standing;

/// A plugin whose one service is first-run by reading the key it writes, claiming a
/// capability with a probe asked as nobody and one asked as the operator.
pub(in crate::plugin::recording) const SERVED: &str = r#"
schema_version = 1

[plugin]
id          = "sonarr"
name        = "Sonarr"
version     = "1.0.0"
description = "Watches for new episodes and fetches them"
without_it  = "Find and download episodes yourself"
upstream    = "https://github.com/Sonarr/Sonarr"
license     = "GPL-3.0-only"
forms       = ["tv", "auto", "full"]

[[service]]
id          = "sonarr"
name        = "Sonarr"
image       = "lscr.io/linuxserver/sonarr"
digest      = "sha256:dffc730adcb8b4f4342792fbb27fcad9c62fb8660523f2aeb082416980d7fe0c"
tag         = "4.0.20.3014-ls327"
port        = 8989
bind        = "loopback"
listens     = 8989
health      = { kind = "http", path = "/ping", timeout_s = 2 }
criticality = "core"
memory_mib  = 300
reaches     = "television metadata providers"
asks_for    = "Reads series, season and episode information."
provides    = ["library.curate"]
api         = { kind = "servarr", key_source = "config-xml", path = "/config/config.xml", version = 3 }
takes_data  = true
user_mode   = "puid-env"
media_types = ["tv"]
replaces    = "sonarr"

[[claim]]
capability = "library.curate"

[[claim.probe]]
id      = "guarded"
request = { method = "GET", path = "/api/v3/wanted/missing" }
expect  = { status = 401 }
fixture = "recordings/sonarr/library-curate-guarded.json"

[[claim.probe]]
id      = "wanted"
request = { method = "GET", path = "/api/v3/wanted/missing" }
expect  = { status = 200, json_types = { records = "list", totalRecords = "int" } }
fixture = "recordings/sonarr/library-curate-wanted.json"
"#;

const PINNED: &str = "lscr.io/linuxserver/sonarr@sha256:dffc730adcb8b4f4342792fbb27fcad9c62fb8660523f2aeb082416980d7fe0c";
const GUARDED: &str = "recordings/sonarr/library-curate-guarded.json";
const WANTED: &str = "recordings/sonarr/library-curate-wanted.json";
const KEY: &str = "0123456789abcdef0123456789abcdef";

fn source(name: &str, manifest: &str) -> Scratch {
    let at = Scratch::new(name);
    let _ = std::fs::write(at.join("plugin.toml"), manifest);
    at
}

fn sonarr() -> Serves {
    Serves::on(49153).writing(
        "/config/config.xml",
        &format!("<Config><ApiKey>{KEY}</ApiKey></Config>"),
    )
}

fn answering(wanted: &str) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        ("/ping", vec![Answer::reply(200, "pong")]),
        (
            "/api/v3/wanted/missing",
            vec![
                Answer::served(401, "text/plain", "Unauthorized"),
                Answer::served(200, "application/json; charset=utf-8", wanted),
            ],
        ),
    ])
}

async fn recorded(at: &Path, serving: &Serves, http: &Arc<Fake>) -> Option<Recorded> {
    let program = Proving::unreachable();
    let shared = || -> Arc<dyn Http> { Arc::clone(http) as Arc<dyn Http> };
    record(
        at,
        Recorder {
            program: program.as_ref(),
            serving,
            transport: &shared,
            random: &FixedRandom(Some(vec![7; 32])),
            taking: &crate::plugin::Retaking::default(),
        },
    )
    .await
    .ok()
}

fn json(at: &Path, fixture: &str) -> serde_json::Value {
    std::fs::read_to_string(at.join(fixture))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn field(value: &serde_json::Value, pointer: &str) -> serde_json::Value {
    value.pointer(pointer).cloned().unwrap_or_default()
}

fn took(read: Option<&Recorded>) -> Vec<Took> {
    read.map(|read| read.recordings.iter().map(|one| one.took.clone()).collect())
        .unwrap_or_default()
}

fn noted(at: &Path, fixture: &str, note: &str) {
    let path = at.join(fixture);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        path,
        format!(
            "{{\"recorded_from\": \"old@sha256:1\", \"note\": \"{note}\", \"request\": {{\"method\": \"GET\", \"path\": \"/x\"}}, \"response\": {{\"status\": 500}}}}"
        ),
    );
}

#[tokio::test]
async fn every_request_is_asked_of_one_fresh_container_after_its_first_run_and_written() {
    let at = source("asking-written", SERVED);
    noted(&at, GUARDED, "Refused without the key.");
    noted(&at, WANTED, "Read with the key it wrote.");
    let serving = sonarr();
    let http = answering("{\"page\": 1, \"records\": [], \"totalRecords\": 0}");
    let read = recorded(&at, &serving, &http).await;
    assert_eq!(took(read.as_ref()), vec![Took::Written, Took::Written]);
    assert!(read
        .as_ref()
        .is_some_and(|read| read.holds && read.failed.is_empty()));

    let guarded = json(&at, GUARDED);
    assert_eq!(field(&guarded, "/recorded_from"), PINNED);
    assert_eq!(field(&guarded, "/note"), "Refused without the key.");
    assert_eq!(
        field(&guarded, "/request"),
        serde_json::json!({"method": "GET", "path": "/api/v3/wanted/missing"})
    );
    assert_eq!(field(&guarded, "/response/status"), 401);
    assert_eq!(
        field(&guarded, "/response/headers/content-type"),
        "text/plain"
    );
    assert_eq!(
        field(&guarded, "/response/body_starts_with"),
        "Unauthorized"
    );
    let wanted = json(&at, WANTED);
    assert_eq!(field(&wanted, "/response/status"), 200);
    assert_eq!(field(&wanted, "/response/json/totalRecords"), 0);

    let asked: Vec<_> = http
        .requests()
        .into_iter()
        .filter(|one| one.url.ends_with("/api/v3/wanted/missing"))
        .collect();
    assert_eq!(asked.len(), 2);
    assert!(asked
        .iter()
        .all(|one| one.url.starts_with("http://127.0.0.1:49153/")));
    let host = ("Host".to_owned(), "127.0.0.1:8989".to_owned());
    assert!(asked
        .first()
        .is_some_and(|one| one.headers == vec![host.clone()]));
    assert!(asked.get(1).is_some_and(
        |one| one.headers == vec![("X-Api-Key".to_owned(), KEY.to_owned()), host.clone()]
    ));

    let served = serving.asked();
    assert_eq!(served.len(), 1, "one container serves every request");
    let Some(served) = served.first() else {
        return;
    };
    assert_eq!(served.image, PINNED);
    assert_eq!(served.listens, Some(8989));
    assert!(served.env.contains(&"PUID=1000".to_owned()));
    assert_eq!(
        served
            .binds
            .iter()
            .map(|bound| bound.target.as_str())
            .collect::<Vec<_>>(),
        vec!["/config", "/data"]
    );
    assert!(
        served.binds.iter().all(|bound| !bound.source.exists()),
        "the copies are gone"
    );
    assert_eq!(serving.removed().len(), 1);
}

#[tokio::test]
async fn a_request_naming_no_fixture_is_not_asked_and_the_rest_are_taken() {
    let at = source(
        "asking-unfixtured",
        &format!(
            "{SERVED}
[[proof]]
id      = \"sonarr.pings\"
title   = \"It answers\"
request = {{ method = \"GET\", path = \"/ping\" }}
expect  = {{ status = 200, json_is_absent = true }}
why     = \"It is there.\"
"
        ),
    );
    let http = answering("{\"records\": [], \"totalRecords\": 0}");
    let read = recorded(&at, &sonarr(), &http).await;
    let took = took(read.as_ref());
    assert!(
        matches!(took.first(), Some(Took::Unrecorded { why }) if why.contains("names no fixture")),
        "{took:?}"
    );
    assert_eq!(took.get(1..), Some(&[Took::Unnoted, Took::Unnoted][..]));
}

#[tokio::test]
async fn recording_again_what_a_fresh_container_answers_the_same_leaves_each_as_it_was() {
    let at = source("asking-again", SERVED);
    noted(&at, GUARDED, "Refused without the key.");
    noted(&at, WANTED, "Read with the key it wrote.");
    let wanted = "{\"page\": 1, \"records\": [], \"totalRecords\": 0}";
    let first = recorded(&at, &sonarr(), &answering(wanted)).await;
    assert_eq!(took(first.as_ref()), vec![Took::Written, Took::Written]);
    let again = recorded(&at, &sonarr(), &answering(wanted)).await;
    assert_eq!(
        took(again.as_ref()),
        vec![Took::Reproduced, Took::Reproduced]
    );
    assert!(again.is_some_and(|read| read.holds));
}

/// A plugin whose one service has no adapter and keeps no library, proved by a request.
fn plain() -> String {
    let service = SERVED.split("[[claim]]").next().unwrap_or_default();
    let kept: Vec<&str> = service
        .lines()
        .filter(|line| {
            !["provides", "api ", "takes_data", "media_types"]
                .iter()
                .any(|dropped| line.starts_with(dropped))
        })
        .collect();
    format!(
        "{}\n[[proof]]\nid      = \"sonarr.status\"\ntitle   = \"It refuses\"\n\
         request = {{ method = \"GET\", path = \"/api/v3/system/status\" }}\n\
         expect  = {{ status = 401, json_is_absent = true }}\n\
         fixture = \"recordings/sonarr/status.json\"\nwhy     = \"It is there.\"\n",
        kept.join("\n")
    )
}

#[tokio::test]
async fn a_service_that_publishes_no_port_fails_at_its_first_request_and_keeps_no_library() {
    let at = source("asking-unpublished", &plain());
    let serving = Serves::up(crate::ports::serving::Served {
        port: None,
        ..served_on_loopback()
    });
    let read = recorded(&at, &serving, &answering("{}")).await;
    assert!(
        read.as_ref()
            .is_some_and(|read| matches!(read.failed.first(),
            Some(Failed { step, why, .. })
                if step == "asking proof sonarr.status" && why.contains("publishes no port"))),
        "{read:?}"
    );
    assert!(serving
        .asked()
        .first()
        .is_some_and(|served| served.binds.len() == 1));
    assert!(!at.join("recordings/sonarr/status.json").exists());
}

/// What the scripted engine says a container it served is known by.
fn served_on_loopback() -> crate::ports::serving::Served {
    crate::ports::serving::Served {
        container: "4f1c2d3e5a6b7c8d9e0f".to_owned(),
        network: "lemonfiber-record-1-1-0".to_owned(),
        hostname: "4f1c2d3e5a6b".to_owned(),
        addresses: Vec::new(),
        machine: Vec::new(),
        port: Some(49153),
    }
}

#[tokio::test]
async fn a_recording_with_no_note_yet_is_written_with_an_empty_one_and_fails_the_run() {
    let at = source("asking-unnoted", SERVED);
    let read = recorded(&at, &sonarr(), &answering("{\"records\": []}")).await;
    assert_eq!(took(read.as_ref()), vec![Took::Unnoted, Took::Unnoted]);
    assert!(read.is_some_and(|read| !read.holds));
    assert_eq!(field(&json(&at, WANTED), "/note"), "");
}

#[tokio::test]
async fn the_credential_and_the_containers_identity_are_written_as_redacted() {
    let at = source("asking-scrubbed", SERVED);
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, KEY);
    let echo = format!(
        "{{\"records\": [\"{}\", \"{encoded}\", \"4f1c2d3e5a6b\", \"172.18.0.2\"], \"{KEY}\": 1}}",
        KEY.to_uppercase()
    );
    let read = recorded(&at, &sonarr(), &answering(&echo)).await;
    assert!(read.is_some_and(|read| read.failed.is_empty()));
    let text = std::fs::read_to_string(at.join(WANTED)).unwrap_or_default();
    assert!(!text.to_lowercase().contains(KEY), "{text}");
    assert!(
        !text.contains(&encoded) && !text.contains("4f1c2d3e5a6b") && !text.contains("172.18.0.2")
    );
    assert_eq!(
        field(&json(&at, WANTED), "/response/json/records"),
        serde_json::json!(["<redacted>", "<redacted>", "<redacted>", "<redacted>"])
    );
}

#[tokio::test]
async fn an_answer_still_holding_a_credential_once_scrubbed_writes_nothing() {
    let at = source("asking-unscrubbable", SERVED);
    let serving = Serves::on(49153).writing(
        "/config/config.xml",
        "<Config><ApiKey>123456789012</ApiKey></Config>",
    );
    let read = recorded(
        &at,
        &serving,
        &answering("{\"records\": [], \"id\": 123456789012}"),
    )
    .await;
    assert!(read
        .as_ref()
        .is_some_and(|read| matches!(read.failed.first(),
            Some(Failed { step, .. }) if step == "scrubbing claim library.curate probe wanted")));
    assert!(!at.join(GUARDED).exists() && !at.join(WANTED).exists());
}

#[tokio::test]
async fn a_service_that_fails_anywhere_writes_none_of_its_recordings_and_is_removed() {
    let at = source("asking-failed", SERVED);
    noted(&at, GUARDED, "kept");
    let before = std::fs::read_to_string(at.join(GUARDED)).unwrap_or_default();
    let serving = sonarr()
        .standing(Standing::Exited(3))
        .logging("starting\n\u{1b}[31mcrashed in 4f1c2d3e5a6b7c8d9e0f\n");
    let read = recorded(&at, &serving, &answering("{}")).await;
    let Some(read) = read else {
        return;
    };
    assert!(!read.holds);
    assert_eq!(
        read.failed,
        vec![Failed {
            service: "sonarr".to_owned(),
            step: "health".to_owned(),
            why: "it exited 3 before its health passed".to_owned(),
            log: vec![
                "starting".to_owned(),
                "[31mcrashed in <redacted>".to_owned(),
            ],
        }]
    );
    assert!(read.recordings.iter().all(|one| matches!(&one.took,
        Took::Unrecorded { why } if why == "sonarr was not recorded: it failed at health")));
    assert_eq!(
        std::fs::read_to_string(at.join(GUARDED)).unwrap_or_default(),
        before
    );
    assert!(!at.join(WANTED).exists());
    assert_eq!(serving.removed().len(), 1);
}

#[tokio::test]
async fn a_service_whose_health_never_passes_fails_at_its_own_limit() {
    let at = source("asking-unhealthy", SERVED);
    let http = Fake::by_path(vec![("/ping", Answer::reply(503, "starting"))]);
    let read = recorded(&at, &sonarr(), &http).await;
    assert!(read.is_some_and(|read| matches!(read.failed.first(),
        Some(Failed { step, why, .. }) if step == "health" && why == "its health did not pass within 2 seconds")));
}

#[tokio::test]
async fn a_service_the_engine_will_not_say_the_standing_of_fails_at_its_health() {
    let at = source("asking-unstanding", SERVED);
    let serving = sonarr().unstanding(Unran::Unreachable {
        reason: "the engine went away".to_owned(),
    });
    let read = recorded(&at, &serving, &answering("{}")).await;
    assert!(read.is_some_and(|read| matches!(read.failed.first(),
        Some(Failed { step, why, .. }) if step == "health" && why.contains("the engine went away"))));
    assert_eq!(serving.removed().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_request_the_service_never_answers_fails_at_it() {
    let at = source("asking-unanswered", SERVED);
    let http = Fake::by_path(vec![("/ping", Answer::reply(200, "pong"))]);
    let read = recorded(&at, &sonarr(), &http).await;
    assert!(read.is_some_and(|read| matches!(read.failed.first(),
        Some(Failed { step, why, .. }) if step == "asking claim library.curate probe guarded"
            && why.starts_with("it did not answer within 30 seconds"))));
    assert!(!at.join(GUARDED).exists());
}

#[tokio::test]
async fn a_service_the_engine_cannot_serve_fails_at_its_container_and_asks_nothing() {
    let at = source("asking-unserved", SERVED);
    let serving = Serves::refusing(Unran::NoImage {
        image: PINNED.to_owned(),
    });
    let http = answering("{}");
    let read = recorded(&at, &serving, &http).await;
    assert!(read.is_some_and(|read| matches!(read.failed.first(),
        Some(Failed { step, why, .. }) if step == "container" && why.contains("pulls no image"))));
    assert!(http.requests().is_empty());
    assert!(serving.removed().is_empty());
}

#[tokio::test]
async fn a_service_with_no_first_run_lemonfiber_can_perform_fails_at_it() {
    let at = source(
        "asking-generated",
        &SERVED.replace(
            "api         = { kind = \"servarr\", key_source = \"config-xml\", path = \"/config/config.xml\", version = 3 }",
            "api         = { kind = \"jellyfin\", key_source = \"generated\" }",
        ),
    );
    let read = recorded(&at, &sonarr(), &answering("{}")).await;
    assert!(read.is_some_and(|read| matches!(read.failed.first(),
        Some(Failed { step, .. }) if step == "first run")));
}

#[tokio::test]
async fn an_operators_probe_on_a_service_no_first_run_leaves_a_credential_for_fails_at_it() {
    let at = source(
        "asking-unkeyed",
        &SERVED.replace(
            "api         = { kind = \"servarr\", key_source = \"config-xml\", path = \"/config/config.xml\", version = 3 }\n",
            "",
        ),
    );
    let http = answering("{}");
    let read = recorded(&at, &sonarr(), &http).await;
    assert!(read.as_ref().is_some_and(|read| matches!(read.failed.first(),
        Some(Failed { step, why, .. }) if step == "first run" && why.contains("claim library.curate probe wanted"))));
    assert!(!http
        .requests()
        .iter()
        .any(|asked| asked.url.ends_with("/api/v3/wanted/missing")));
    assert!(!at.join(GUARDED).exists());
}

#[cfg(unix)]
#[tokio::test]
async fn a_recording_that_would_be_written_through_a_link_stops_the_service_before_it_starts() {
    let elsewhere = Scratch::new("asking-elsewhere");
    let at = source("asking-linked", SERVED);
    let _ = std::os::unix::fs::symlink(elsewhere.path(), at.join("recordings"));
    let serving = sonarr();
    let read = recorded(&at, &serving, &answering("{}")).await;
    assert!(read.is_some_and(|read| matches!(read.failed.first(),
        Some(Failed { step, why, .. }) if *step == format!("writing {GUARDED}") && why.contains("is a link"))));
    assert!(serving.asked().is_empty());
}
