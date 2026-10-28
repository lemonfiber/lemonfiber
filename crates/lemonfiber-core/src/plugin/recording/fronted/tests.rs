use std::path::Path;
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::proving::Proving;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_fixtures::serving::Serves;
use lemonfiber_fixtures::support::FixedRandom;
use serde_json::{json, Value};

use super::super::{record, Failed, Recorded, Recorder, Took};
use crate::ports::http::{Http, Request};
use crate::ports::program::Unran;
use crate::ports::serving::Served;

const FRONTED: &str = r#"
schema_version = 1

[plugin]
id          = "jellyfin"
name        = "Jellyfin"
version     = "1.0.0"
description = "Plays your library on TVs, phones and browsers"
without_it  = "Files on disk, no way to watch them"
upstream    = "https://github.com/jellyfin/jellyfin"
license     = "GPL-2.0-only"
forms       = ["library", "full"]

[[service]]
id          = "jellyfin"
name        = "Jellyfin"
image       = "docker.io/jellyfin/jellyfin"
digest      = "sha256:aefb67e6a7ff1debdd154a78a7bbb780fd0c873d8639210a7f6a2016ad2b35db"
tag         = "10.11.11"
port        = 8095
bind        = "loopback"
health      = { kind = "http", path = "/health", timeout_s = 2 }
criticality = "important"
memory_mib  = 600
reaches     = "metadata providers"
asks_for    = "Reads artwork and descriptions for what is in your library."
takes_data  = true
config_path = "/config"
listens     = 8096
user_mode   = "uid"
native      = "jellyfin"
replaces    = "jellyfin"

[[service]]
id          = "jellyfin-adapter"
name        = "Jellyfin adapter"
image       = "ghcr.io/lemonfiber/jellyfin-adapter"
digest      = "sha256:0352d3232d0ffe22725ebd706ed8fa92d23b20de02a076ac195971d1fc4a0c07"
tag         = "main-31a58cdb60de"
criticality = "important"
reaches     = ""
asks_for    = "Nothing beyond this machine."
config_path = "/config"
listens     = 8080
health      = { kind = "http", path = "/lemonfiber/adapter/v1/ready", timeout_s = 2 }
speaks      = ["subtitles.fetch@1"]
fronts      = "jellyfin"
provides    = ["subtitles.fetch"]

[[proof]]
id      = "jellyfin.identifies"
service = "jellyfin"
title   = "Jellyfin says which server it is to anybody"
fixture = "fixtures/system-info-public.json"
why     = "The adapter's readiness is this answer."
request = { method = "GET", path = "/System/Info/Public" }
expect  = { status = 200, json = { ProductName = "Jellyfin Server" } }
"#;

const PINNED: &str = "docker.io/jellyfin/jellyfin@sha256:aefb67e6a7ff1debdd154a78a7bbb780fd0c873d8639210a7f6a2016ad2b35db";
const PROVED: &str = "fixtures/system-info-public.json";
const WATCH: &str = "conformance/subtitles.fetch@1/watch-answers.json";
const WATCHING: &str = "conformance/subtitles.fetch@1/watching-answers.json";
const REFUSES: &str = "conformance/subtitles.fetch@1/refuses-without-the-key.json";
const MINTED: &str = "0707070707070707070707070707070707070707070707070707070707070707";

fn source(name: &str, manifest: &str) -> Scratch {
    let at = Scratch::new(name);
    let _ = std::fs::write(at.join("plugin.toml"), manifest);
    at
}

fn adapter(port: Option<u16>) -> Served {
    Served {
        container: "9a8b7c6d5e4f3a2b1c0d".to_owned(),
        network: "lemonfiber-record-1-1-0".to_owned(),
        hostname: "9a8b7c6d5e4f".to_owned(),
        addresses: vec!["17218003".to_owned()],
        machine: vec!["recording-host".to_owned()],
        port,
        contract: None,
    }
}

fn engine() -> Serves {
    Serves::on(49153).besides(Ok(adapter(Some(49154))))
}

fn fetching(watching: Vec<Answer>, watch: Answer) -> Arc<Fake> {
    settling(vec![Answer::reply(200, "true")], watching, watch)
}

fn settling(ready: Vec<Answer>, watching: Vec<Answer>, watch: Answer) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        ("/health", vec![Answer::reply(200, "ok")]),
        ("/lemonfiber/adapter/v1/ready", ready),
        (
            "/System/Info/Public",
            vec![Answer::served(
                200,
                "application/json",
                "{\"ProductName\": \"Jellyfin Server\"}",
            )],
        ),
        ("/watching", watching),
        ("/watch", vec![watch]),
    ])
}

fn answering() -> Arc<Fake> {
    fetching(
        vec![
            Answer::served(401, "text/plain", "Unauthorized"),
            Answer::served(200, "application/json", "[]"),
        ],
        Answer::served(200, "application/json", "null"),
    )
}

async fn read(
    at: &Path,
    serving: &Serves,
    http: &Arc<Fake>,
    random: Option<u8>,
) -> Result<Recorded, super::super::Unreadable> {
    read_through(at, serving, Arc::clone(http) as Arc<dyn Http>, random).await
}

async fn read_through(
    at: &Path,
    serving: &Serves,
    http: Arc<dyn Http>,
    random: Option<u8>,
) -> Result<Recorded, super::super::Unreadable> {
    let program = Proving::unreachable();
    let shared = || -> Arc<dyn Http> { Arc::clone(&http) };
    record(
        at,
        Recorder {
            program: program.as_ref(),
            serving,
            transport: &shared,
            random: &FixedRandom(random.map(|byte| vec![byte; 32])),
            taking: &crate::plugin::Retaking::default(),
            confined: &lemonfiber_adapters::Disk,
        },
    )
    .await
}

async fn recorded(at: &Path, serving: &Serves, http: &Arc<Fake>, random: Option<u8>) -> Recorded {
    let read = read(at, serving, http, random).await;
    assert!(
        read.as_ref().is_ok_and(|read| read.refusals.is_empty()),
        "{read:?}"
    );
    read.unwrap_or_else(|_| unreachable!("asserted to be read above"))
}

fn json_at(at: &Path, fixture: &str) -> Value {
    std::fs::read_to_string(at.join(fixture))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn posted(http: &Fake, fragment: &str) -> Vec<Request> {
    http.requests()
        .into_iter()
        .filter(|request| request.url.ends_with(fragment))
        .collect()
}

fn body(request: &Request) -> Value {
    request
        .body
        .as_deref()
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default()
}

fn bearer(request: &Request) -> Option<&str> {
    request
        .headers
        .iter()
        .find(|(name, _)| name == "Authorization")
        .map(|(_, value)| value.as_str())
}

fn failed_at(read: &Recorded) -> Vec<(String, String)> {
    read.failed
        .iter()
        .map(|Failed { step, why, .. }| (step.clone(), why.clone()))
        .collect()
}

#[tokio::test]
async fn every_case_the_adapter_speaks_is_asked_of_it_beside_its_upstream_and_written() {
    let at = source("fronted-asked", FRONTED);
    let serving = engine();
    let http = answering();
    let read = recorded(&at, &serving, &http, Some(7)).await;
    assert_eq!(failed_at(&read), Vec::new());
    let named: Vec<(String, Took)> = read
        .recordings
        .iter()
        .map(|taken| (taken.named.clone(), taken.took.clone()))
        .collect();
    assert_eq!(
        named,
        vec![
            ("proof jellyfin.identifies".to_owned(), Took::Unnoted),
            (
                "conformance subtitles.fetch@1/refuses-without-the-key".to_owned(),
                Took::Unnoted
            ),
            (
                "conformance subtitles.fetch@1/watching-answers".to_owned(),
                Took::Unnoted
            ),
            (
                "conformance subtitles.fetch@1/watch-answers".to_owned(),
                Took::Unnoted
            ),
        ]
    );
    let watch = json_at(&at, WATCH);
    assert_eq!(watch.pointer("/recorded_from"), Some(&json!(PINNED)));
    assert_eq!(
        watch.pointer("/request"),
        Some(&json!({
            "method": "POST",
            "path": "/lemonfiber/subtitles.fetch/v1/watch",
            "accept": "application/json",
        }))
    );
    assert_eq!(
        watch.pointer("/response/body_starts_with"),
        Some(&json!("null"))
    );
    assert_eq!(
        json_at(&at, REFUSES).pointer("/response/status"),
        Some(&json!(401))
    );
    assert_eq!(
        json_at(&at, WATCHING).pointer("/response/json"),
        Some(&json!([]))
    );
    assert_eq!(
        json_at(&at, PROVED).pointer("/response/json/ProductName"),
        Some(&json!("Jellyfin Server"))
    );
    let watching = posted(&http, "/watching");
    let keys: Vec<Option<&str>> = watching.iter().map(bearer).collect();
    let presented = format!("Bearer {MINTED}");
    assert_eq!(keys, vec![None, Some(presented.as_str())]);
    let watched = posted(&http, "/watch");
    assert_eq!(
        watched.first().map(body),
        Some(json!({"watched": {
            "api_key": MINTED,
            "host": "conformance-curator",
            "port": 58989,
            "which": "tv",
        }}))
    );
    let served = serving.asked();
    let aliases: Vec<Option<&str>> = served.iter().map(|one| one.alias.as_deref()).collect();
    assert_eq!(aliases, vec![Some("jellyfin"), None]);
    assert_eq!(
        served.get(1).map(|one| one.image.as_str()),
        Some("ghcr.io/lemonfiber/jellyfin-adapter@sha256:0352d3232d0ffe22725ebd706ed8fa92d23b20de02a076ac195971d1fc4a0c07")
    );
    let removed: Vec<String> = serving
        .removed()
        .into_iter()
        .map(|one| one.container)
        .collect();
    assert_eq!(
        removed,
        vec![
            "9a8b7c6d5e4f3a2b1c0d".to_owned(),
            "4f1c2d3e5a6b7c8d9e0f".to_owned()
        ]
    );
}

#[tokio::test]
async fn an_adapter_that_cannot_be_started_leaves_every_case_unrecorded_and_its_upstream_removed() {
    let at = source("fronted-unstarted", FRONTED);
    let serving = Serves::on(49153).besides(Err(Unran::Unreachable {
        reason: "no engine".to_owned(),
    }));
    let read = recorded(&at, &serving, &answering(), Some(7)).await;
    assert_eq!(
        read.failed
            .iter()
            .map(|failed| failed.step.as_str())
            .collect::<Vec<_>>(),
        vec!["adapter's container"]
    );
    assert!(read.recordings.iter().all(|taken| matches!(
        &taken.took,
        Took::Unrecorded { why } if why == "jellyfin was not recorded: it failed at adapter's container"
    )));
    assert_eq!(read.recordings.len(), 4);
    assert_eq!(serving.removed().len(), 1);
    assert!(!at.join(WATCH).exists() && !at.join(PROVED).exists());
}

#[tokio::test(start_paused = true)]
async fn an_adapter_saying_its_upstream_does_not_answer_fails_at_its_health_and_both_are_removed() {
    let at = source("fronted-unready", FRONTED);
    let serving = engine();
    let http = Fake::by_path(vec![
        ("/health", Answer::reply(200, "ok")),
        ("/ready", Answer::reply(200, "false")),
    ]);
    let read = recorded(&at, &serving, &http, Some(7)).await;
    assert_eq!(
        failed_at(&read),
        vec![(
            "adapter's health".to_owned(),
            "it did not say its upstream answers within 2 seconds".to_owned()
        )]
    );
    assert!(http
        .requests()
        .iter()
        .filter(|request| request.url.ends_with("/lemonfiber/adapter/v1/ready"))
        .all(|request| request
            .headers
            .iter()
            .any(|(name, value)| name == "Authorization" && value.starts_with("Bearer "))));
    assert_eq!(serving.removed().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn an_adapter_is_asked_nothing_until_it_says_its_upstream_answers() {
    let at = source("fronted-settling", FRONTED);
    let http = settling(
        vec![Answer::reply(200, "false"), Answer::reply(200, "true")],
        vec![Answer::served(200, "application/json", "[]")],
        Answer::served(200, "application/json", "null"),
    );
    let read = recorded(&at, &engine(), &http, Some(7)).await;
    assert_eq!(failed_at(&read), Vec::new());
    let asked: Vec<String> = http
        .requests()
        .into_iter()
        .map(|request| request.url)
        .filter(|url| url.contains(":49154/"))
        .collect();
    let waited = asked
        .iter()
        .take_while(|url| url.ends_with("/lemonfiber/adapter/v1/ready"))
        .count();
    assert_eq!(waited, 2, "{asked:?}");
    assert!(asked.len() > waited, "{asked:?}");
    let presented = format!("Bearer {MINTED}");
    let readies = posted(&http, "/lemonfiber/adapter/v1/ready");
    let keys: Vec<Option<&str>> = readies.iter().map(bearer).collect();
    assert_eq!(keys, vec![Some(presented.as_str()); 2]);
}

#[tokio::test(start_paused = true)]
async fn an_adapter_refusing_its_key_fails_at_its_health_without_waiting_out_its_limit() {
    let at = source("fronted-unkeyed", FRONTED);
    let serving = engine();
    let http = Fake::by_path(vec![
        ("/health", Answer::reply(200, "ok")),
        ("/ready", Answer::reply(401, "")),
    ]);
    let started = tokio::time::Instant::now();
    let read = recorded(&at, &serving, &http, Some(7)).await;
    let failed = failed_at(&read);
    assert!(
        matches!(failed.as_slice(), [(step, why)]
            if step == "adapter's health" && why.ends_with("rejected the credential")),
        "{failed:?}"
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    assert_eq!(serving.removed().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn an_upstream_never_healthy_fails_at_its_health_and_both_are_removed() {
    let at = source("fronted-unhealthy", FRONTED);
    let serving = Serves::on(49153);
    let http = Fake::by_path(vec![("/health", Answer::reply(503, "starting"))]);
    let read = recorded(&at, &serving, &http, Some(7)).await;
    assert_eq!(
        failed_at(&read).first().map(|(step, _)| step.as_str()),
        Some("health")
    );
    assert_eq!(serving.asked().len(), 2);
    assert_eq!(serving.removed().len(), 2);
}

#[tokio::test]
async fn an_adapter_or_upstream_publishing_no_port_cannot_be_asked() {
    let at = source("fronted-portless-adapter", FRONTED);
    let serving = Serves::on(49153).besides(Ok(adapter(None)));
    let health = FRONTED.replace(
        "health      = { kind = \"http\", path = \"/lemonfiber/adapter/v1/ready\", timeout_s = 2 }\n",
        "",
    );
    let _ = std::fs::write(at.join("plugin.toml"), &health);
    let http = Fake::always(Answer::reply(200, "true"));
    let read = recorded(&at, &serving, &http, Some(7)).await;
    assert_eq!(
        failed_at(&read),
        vec![(
            "adapter's health".to_owned(),
            "it publishes no port, so nothing can ask it".to_owned()
        )]
    );
    let at = source(
        "fronted-portless-upstream",
        &health.replacen(
            "health      = { kind = \"http\", path = \"/health\", timeout_s = 2 }\n",
            "",
            1,
        ),
    );
    let serving = Serves::up(Served {
        port: None,
        ..adapter(None)
    })
    .besides(Ok(adapter(Some(49154))));
    let read = recorded(&at, &serving, &http, Some(7)).await;
    assert_eq!(
        failed_at(&read),
        vec![(
            "asking proof jellyfin.identifies".to_owned(),
            "it publishes no port, so there is nowhere to ask it".to_owned()
        )]
    );
}

#[tokio::test]
async fn an_upstream_that_cannot_be_started_starts_no_adapter() {
    let at = source("fronted-refused", FRONTED);
    let serving = Serves::refusing(Unran::Unreachable {
        reason: "no engine".to_owned(),
    });
    let read = recorded(&at, &serving, &answering(), Some(7)).await;
    assert_eq!(
        failed_at(&read).first().map(|(step, _)| step.as_str()),
        Some("container")
    );
    assert_eq!(serving.asked().len(), 1);
    assert!(serving.removed().is_empty());
}

#[tokio::test]
async fn a_case_the_adapter_does_not_answer_fails_the_service_naming_the_case() {
    let at = source("fronted-silent", FRONTED);
    let serving = engine().logging("adapter 9a8b7c6d5e4f gone");
    let http = fetching(
        vec![Answer::served(200, "application/json", "[]")],
        Answer::Silent,
    );
    let read = recorded(&at, &serving, &http, Some(7)).await;
    let failed = read.failed.first();
    assert_eq!(
        failed.map(|failed| failed.step.as_str()),
        Some("conformance subtitles.fetch@1/watch-answers")
    );
    assert_eq!(
        failed.map(|failed| failed.log.clone()),
        Some(vec!["adapter <redacted> gone".to_owned()])
    );
    assert!(!at.join(WATCHING).exists());
}

#[tokio::test]
async fn an_answer_still_holding_the_adapters_identity_once_scrubbed_writes_nothing() {
    let at = source(
        "fronted-unscrubbable",
        &FRONTED.replace("takes_data  = true\n", ""),
    );
    let http = fetching(
        vec![Answer::served(
            200,
            "application/json",
            "{\"at\": 17218003}",
        )],
        Answer::served(200, "application/json", "null"),
    );
    let read = recorded(&at, &engine(), &http, Some(7)).await;
    assert_eq!(
        failed_at(&read).first().map(|(step, _)| step.as_str()),
        Some("scrubbing")
    );
    assert!(!at.join(WATCHING).exists() && !at.join(PROVED).exists());
}

#[tokio::test(start_paused = true)]
async fn an_upstream_request_never_answered_fails_at_it_and_asks_the_adapter_nothing() {
    let at = source("fronted-unanswered", FRONTED);
    let http = Fake::by_path_in_turn(vec![
        ("/health", vec![Answer::reply(200, "ok")]),
        (
            "/lemonfiber/adapter/v1/ready",
            vec![Answer::reply(200, "true")],
        ),
    ]);
    let read = recorded(&at, &engine(), &http, Some(7)).await;
    let failed = failed_at(&read);
    assert!(
        matches!(failed.first(), Some((step, why)) if step == "asking proof jellyfin.identifies"
            && why.starts_with("it did not answer within 30 seconds")),
        "{failed:?}"
    );
    assert!(posted(&http, "/watching").is_empty());
}

#[tokio::test]
async fn with_no_randomness_there_is_no_key_for_the_adapter_and_nothing_is_started() {
    let at = source("fronted-keyless", FRONTED);
    let serving = engine();
    let read = recorded(&at, &serving, &answering(), None).await;
    assert_eq!(
        failed_at(&read),
        vec![(
            "adapter's key".to_owned(),
            crate::secret::NO_RANDOMNESS_FOR_PASSWORD.to_owned()
        )]
    );
    assert!(serving.asked().is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn starting_files_holding_a_link_are_refused_for_the_upstream_and_for_the_adapter() {
    for (service, step) in [
        ("jellyfin", "starting files"),
        ("jellyfin-adapter", "adapter's starting files"),
    ] {
        let at = source("fronted-linked", FRONTED);
        let starting = at.join("config").join(service);
        let _ = std::fs::create_dir_all(&starting);
        let _ = std::os::unix::fs::symlink("/etc/hosts", starting.join("hosts"));
        let read = recorded(&at, &engine(), &answering(), Some(7)).await;
        assert_eq!(
            failed_at(&read).first().map(|(step, _)| step.as_str()),
            Some(step)
        );
    }
}

const OPERATORS: &str = r#"
[[claim]]
capability = "media.serve"

[[claim.probe]]
id      = "guarded"
request = { method = "GET", path = "/Items" }
expect  = { status = 401 }
fixture = "fixtures/items-unauthenticated.json"

[[claim.probe]]
id      = "catalogue"
request = { method = "GET", path = "/Items" }
expect  = { status = 200, json_has_keys = ["Items", "TotalRecordCount"] }
fixture = "fixtures/items.json"
"#;

#[tokio::test]
async fn a_probe_asked_as_the_operator_of_the_upstream_refuses_the_manifest_and_starts_nothing() {
    let manifest = FRONTED.replace(
        "user_mode   = \"uid\"\n",
        "user_mode   = \"uid\"\nprovides    = [\"media.serve\"]\n",
    );
    let at = source("fronted-operator", &format!("{manifest}{OPERATORS}"));
    let serving = engine();
    let read = read(&at, &serving, &answering(), Some(7)).await;
    assert!(
        read.as_ref()
            .is_ok_and(|read| read.refusals.iter().any(|refusal| refusal.location
                == "claim media.serve.probe catalogue"
                && refusal.message.contains("jellyfin-adapter"))),
        "{read:?}"
    );
    assert!(serving.asked().is_empty());
}

#[tokio::test]
async fn an_adapter_fronting_a_minted_upstream_credential_is_handed_it_and_recorded() {
    let minting = FRONTED.replace(
        "fronts      = \"jellyfin\"\n",
        "fronts      = \"jellyfin\"\nupstream    = { api_key = { secret = \"upstream-key\" } }\n",
    ) + "\n[[secret]]\nid     = \"upstream-key\"\nof     = \"jellyfin\"\nminted = true\nwhy    = \
         \"The key the adapter signs in to Jellyfin with\"\n";
    let at = source("fronted-minted-upstream", &minting);
    let read = recorded(&at, &engine(), &answering(), Some(7)).await;
    assert_eq!(failed_at(&read), Vec::new());
    assert_eq!(read.recordings.len(), 4);
}

#[tokio::test]
async fn an_upstream_given_its_minted_key_in_its_environment_holds_the_one_its_adapter_presents() {
    let minting = FRONTED
        .replace(
            "fronts      = \"jellyfin\"\n",
            "fronts      = \"jellyfin\"\nupstream    = { api_key = { secret = \"upstream-key\" } }\n",
        )
        .replace(
            "replaces    = \"jellyfin\"\n",
            "replaces    = \"jellyfin\"\nenv         = { JELLYFIN_API_KEY = { secret = \"upstream-key\" } }\n",
        )
        + "\n[[secret]]\nid     = \"upstream-key\"\nof     = \"jellyfin\"\nminted = true\nwhy    = \
           \"The key the adapter signs in to Jellyfin with\"\n";
    let at = source("fronted-minted-environment", &minting);
    let serving = engine();
    let read = recorded(&at, &serving, &answering(), Some(7)).await;

    assert_eq!(failed_at(&read), Vec::new());
    let given: Vec<String> = serving
        .asked()
        .first()
        .map(|upstream| upstream.env.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|one| one.starts_with("JELLYFIN_API_KEY="))
        .collect();
    let key = given
        .first()
        .and_then(|one| one.strip_prefix("JELLYFIN_API_KEY="))
        .unwrap_or_default()
        .to_owned();
    assert_eq!(given.len(), 1, "{given:?}");
    assert!(
        !key.is_empty() && key.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{key}"
    );
    assert!(read.recordings.iter().all(|one| !serde_json::to_string(one)
        .unwrap_or_default()
        .contains(&key)));
}

/// The fixture's adapter speaking the curator's contract in place of its own.
fn curating() -> String {
    FRONTED
        .replace(
            "speaks      = [\"subtitles.fetch@1\"]",
            "speaks      = [\"library.curate@1\"]",
        )
        .replace(
            "provides    = [\"subtitles.fetch\"]",
            "provides    = [\"library.curate\"]",
        )
}

#[tokio::test]
async fn a_curator_is_recorded_with_its_download_client_answered_where_its_upstream_reaches_it() {
    let at = source("fronted-curating", &curating());
    let serving = engine();
    let _ = read(&at, &serving, &answering(), Some(7)).await;

    let stood: Vec<u16> = serving.stood_in().iter().map(|one| one.port).collect();
    assert_eq!(stood, vec![8080]);
    assert!(
        serving
            .asked()
            .first()
            .is_some_and(|upstream| upstream.reaching == vec!["conformance-download".to_owned()]),
        "{:?}",
        serving.asked()
    );
}

#[tokio::test]
async fn an_adapter_whose_cases_hand_its_upstream_a_curator_is_recorded_with_one_answering() {
    let at = source("fronted-curator", FRONTED);
    let serving = engine();
    let _ = recorded(&at, &serving, &answering(), Some(7)).await;

    let stood: Vec<u16> = serving.stood_in().iter().map(|one| one.port).collect();
    assert_eq!(stood, vec![58989]);
    assert_eq!(
        serving
            .asked()
            .first()
            .map(|upstream| upstream.reaching.clone()),
        Some(vec!["conformance-curator".to_owned()])
    );
}

#[tokio::test]
async fn an_adapter_whose_cases_name_no_party_is_recorded_with_nothing_answering_beside_it() {
    let at = source(
        "fronted-no-party",
        &FRONTED
            .replace(
                "speaks      = [\"subtitles.fetch@1\"]",
                "speaks      = [\"download.usenet@1\"]",
            )
            .replace(
                "provides    = [\"subtitles.fetch\"]",
                "provides    = [\"download.usenet\"]",
            ),
    );
    let serving = engine();
    let _ = read(&at, &serving, &answering(), Some(7)).await;

    assert!(serving.stood_in().is_empty());
    assert!(serving.asked().iter().all(|one| one.reaching.is_empty()));
}

#[tokio::test]
async fn a_curator_whose_download_client_cannot_answer_fails_at_it_and_starts_nothing() {
    let at = source("fronted-client-unanswered", &curating());
    let serving = engine().unanswering(Unran::Refused {
        reason: "127.0.0.1:8080 could not be taken to answer on".to_owned(),
    });
    let read = recorded(&at, &serving, &answering(), Some(7)).await;

    assert_eq!(
        failed_at(&read)
            .first()
            .map(|(step, why)| (step.as_str(), why.as_str())),
        Some((
            "download client",
            "the container engine refused the run: 127.0.0.1:8080 could not be taken to answer on"
        ))
    );
    assert_eq!(serving.stood_in().len(), 1);
    assert!(serving.asked().is_empty());
}

mod alone;
mod checking;
mod claiming;
mod identity;
mod internal;
mod sharing;
mod standing_in;
mod unoffered;
