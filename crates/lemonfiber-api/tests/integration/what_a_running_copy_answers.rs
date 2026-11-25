use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::to_bytes;
use axum::Extension;
use lemonfiber_api::admission::{Caller, Keyed};
use lemonfiber_api::events::dashboard::Dashboard;
use lemonfiber_api::events::live::{Gathers, Listening, Live};
use lemonfiber_api::events::theirs::Theirs;
use lemonfiber_api::events::wire::{Audience, EventKind, Rendered, Replayed};
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::read::{self, table};
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::archive::{Archive, Archiving, Fault, Reader, Space};
use lemonfiber_core::backup::{self, Existing, Item, Manifest, Scope as Copied};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::copying::pacing::Stopwatch;
use lemonfiber_core::copying::run::Recorder;
use lemonfiber_core::copying::{Meter, Starting, What};
use lemonfiber_core::keys::Scope;
use lemonfiber_fixtures::ports::{Chance, Stopped, Waited, STAMPED};
use lemonfiber_fixtures::scratch::Scratch;
use serde_json::{json, Value};

const MEMBER: &str = "a7f3";

/// An archive nothing here asks of: the copy these tests read is begun by hand.
struct Unasked;

fn unasked() -> Fault {
    Fault::new("these tests read a copy and take none")
}

#[async_trait]
impl Archive for Unasked {
    async fn space(&self, _dir: &Path, _items: &[Item]) -> Result<Space, Fault> {
        Err(unasked())
    }
    async fn write(
        &self,
        _dest: &Path,
        _manifest: &Manifest,
        _items: &[Item],
        _meter: Arc<dyn Meter>,
    ) -> Result<(), Fault> {
        Err(unasked())
    }
    async fn write_files(&self, _dest: &Path, _files: &[(String, String)]) -> Result<(), Fault> {
        Err(unasked())
    }
    async fn existing(&self, _dir: &Path) -> Result<Vec<Existing>, Fault> {
        Err(unasked())
    }
    async fn remove(&self, _dir: &Path, _name: &str) -> Result<(), Fault> {
        Err(unasked())
    }
}

#[async_trait]
impl Reader for Unasked {
    async fn read_manifest(&self, _src: &Path) -> Result<Manifest, Fault> {
        Err(unasked())
    }
    async fn extract(
        &self,
        _src: &Path,
        _targets: &[(String, PathBuf)],
        _meter: Arc<dyn Meter>,
    ) -> Result<(), Fault> {
        Err(unasked())
    }
}

/// A machine keeping its archives in a scratch directory, and those paths.
fn world(named: &str) -> (Ctx, Paths) {
    let scratch = Scratch::named(&format!("copying-web-{named}"));
    let paths = Paths::at(scratch.path(), scratch.path());
    let ctx = lemonfiber_testing::a_context()
        .clock(Stopped::at(STAMPED.seconds()))
        .holding(scratch)
        .build()
        .with_random(Arc::new(Chance::cycling()))
        .with_archives(Archiving {
            paths: paths.clone(),
            vault: Arc::new(Unasked),
        });
    (ctx, paths)
}

/// A backup of the whole stack held to 50 MiB a second, part-way through sonarr's.
fn a_backup_running(paths: &Paths) -> Arc<Recorder> {
    let stopwatch = Waited::started();
    let meter = Starting::on(
        Stopped::at(STAMPED.seconds()),
        Arc::clone(&stopwatch) as Arc<dyn Stopwatch>,
        Some("9f2c1a7e04b3d815".to_owned()),
        Some(50 << 20),
    )
    .kept_under(paths)
    .begin(
        What::Backup,
        Copied::WholeStack,
        2 << 30,
        backup::plan(paths, &Copied::WholeStack, &[]).areas(),
    );
    meter.writing("services/sonarr/sonarr.db");
    stopwatch.pass(Duration::from_secs(14));
    meter.wrote(700 << 20);
    meter
}

fn the_running_backup() -> Value {
    json!({
        "what": "backup",
        "scope": { "scope": "whole_stack" },
        "job": "9f2c1a7e04b3d815",
        "started": STAMPED.seconds(),
        "written": 700 << 20,
        "whole": 2_u64 << 30,
        "part": "sonarr configuration",
        "state": "paced",
        "limit": 50 << 20,
    })
}

/// The host tree the existing setup below keeps its configuration in.
const HOST_PATH: &str = "/srv/media/sonarr";

/// A capture of an existing setup, part-way through its one tree.
fn an_existing_setup_copying(paths: &Paths) -> Arc<Recorder> {
    let scope = Copied::existing("media", &[HOST_PATH.to_owned()]);
    let areas = backup::plan(paths, &scope, &[]).areas();
    let meter = Starting::on(
        Stopped::at(STAMPED.seconds()),
        Waited::started() as Arc<dyn Stopwatch>,
        Some("9f2c1a7e04b3d815".to_owned()),
        None,
    )
    .kept_under(paths)
    .begin(What::Backup, scope, 2 << 30, areas);
    meter.writing("existing/0/config.xml");
    meter
}

/// `running` as a key reads it, where it copies what lemonfiber manages.
fn a_keys_reading(mut running: Value) -> Value {
    if let Some(running) = running.as_object_mut() {
        running.remove("job");
    }
    running
}

fn key(scope: Scope) -> Caller {
    Caller::Key(Keyed {
        name: "a program".to_owned(),
        scope,
    })
}

async fn asked(ctx: Ctx, caller: Caller) -> (u16, Value) {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    let router = read::routes()
        .with_state(Serving {
            ctx: Arc::new(ctx),
            token: Arc::new(token),
            bound: lemonfiber_api::guard::Binding::here(8475),
            jobs: Jobs::default(),
            live: Arc::new(Live::opening(Stopped::at(0))),
            kept: Arc::default(),
            answered: Arc::default(),
        })
        .layer(Extension(caller));
    let Ok(request) = axum::http::Request::builder()
        .method("GET")
        .uri(table::COPYING)
        .body(axum::body::Body::empty())
    else {
        unreachable!("a request built from values that are already headers cannot fail");
    };
    let Some(response) = tower::ServiceExt::oneshot(router, request).await.ok() else {
        unreachable!("the router is infallible; its handlers answer rather than fail");
    };
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .map(|bytes| bytes.to_vec())
        .unwrap_or_default();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

#[tokio::test]
async fn the_operator_reads_the_copy_running_whoever_started_it() {
    for caller in [Caller::Machine, Caller::Operator] {
        let (ctx, paths) = world("answered");
        let _running = a_backup_running(&paths);
        let (status, body) = asked(ctx, caller.clone()).await;
        assert_eq!(status, 200, "{caller:?}: {body}");
        assert_eq!(
            body.get("kind"),
            Some(&json!("copying")),
            "{caller:?}: {body}"
        );
        assert_eq!(
            body.pointer("/data/running"),
            Some(&the_running_backup()),
            "{caller:?}"
        );
    }
}

#[tokio::test]
async fn a_read_or_act_key_reads_the_copy_without_the_job_only_the_operator_redeems() {
    for caller in [key(Scope::Read), key(Scope::Act)] {
        let (ctx, paths) = world("keyed");
        let _running = a_backup_running(&paths);
        let (status, body) = asked(ctx, caller.clone()).await;
        assert_eq!(status, 200, "{caller:?}: {body}");
        assert_eq!(
            body.pointer("/data/running"),
            Some(&a_keys_reading(the_running_backup())),
            "{caller:?}"
        );
    }
}

#[tokio::test]
async fn a_key_reads_a_copy_of_an_existing_setup_without_its_project_trees_or_host_paths() {
    let (ctx, paths) = world("existing");
    let _running = an_existing_setup_copying(&paths);

    let (status, operators) = asked(ctx.clone(), Caller::Operator).await;
    assert_eq!(status, 200, "{operators}");
    assert_eq!(
        operators.pointer("/data/running/scope/project"),
        Some(&json!("media"))
    );
    assert_eq!(
        operators.pointer("/data/running/part"),
        Some(&json!(HOST_PATH))
    );
    assert_eq!(
        operators.pointer("/data/running/scope/trees/0/host_path"),
        Some(&json!(HOST_PATH))
    );

    for caller in [key(Scope::Read), key(Scope::Act)] {
        let (status, body) = asked(ctx.clone(), caller.clone()).await;
        assert_eq!(status, 200, "{caller:?}: {body}");
        let Some(running) = body.pointer("/data/running") else {
            unreachable!("a copy is running");
        };
        assert_eq!(
            running.get("scope"),
            Some(&json!({ "scope": "existing" })),
            "{body}"
        );
        assert_eq!(
            running.get("part"),
            Some(&json!(lemonfiber_core::copying::EXISTING_PART)),
            "{body}"
        );
        assert_eq!(running.get("job"), None, "{body}");
        for withheld in ["media", "/srv", "9f2c1a7e04b3d815"] {
            assert!(!body.to_string().contains(withheld), "{withheld}: {body}");
        }
    }
}

#[tokio::test]
async fn a_client_with_no_credential_is_refused_the_read_and_the_stream() {
    let (ctx, paths) = world("nobody");
    let _running = a_backup_running(&paths);
    let admitting = crate::door::unhoused();

    let (router, _token) = crate::door::surface(ctx.clone(), &admitting);
    let read = crate::door::asked(
        router.clone(),
        "GET",
        table::COPYING,
        &crate::door::from_here(),
        "",
    )
    .await;
    assert_eq!(read.status.as_u16(), 403, "{}", read.body);
    assert!(!read.body.contains("sonarr"), "{}", read.body);

    for stream in [router, crate::door::stream_reading(&admitting, ctx)] {
        let (status, heard) =
            crate::door::listened(stream, &crate::door::from_here(), None, 1).await;
        assert_eq!(status.as_u16(), 403);
        assert!(!heard.iter().any(|kind| kind == "copying"), "{heard:?}");
    }
}

#[tokio::test]
async fn nothing_running_reads_as_none() {
    let (ctx, paths) = world("none");
    drop(a_backup_running(&paths));
    let (status, body) = asked(ctx, Caller::Operator).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body.get("data"), Some(&json!({ "running": null })));
}

#[tokio::test]
async fn a_member_and_a_key_scoped_to_one_are_refused_the_copy() {
    for caller in [
        Caller::Member(MEMBER.to_owned()),
        key(Scope::Member {
            id: MEMBER.to_owned(),
            name: "ana".to_owned(),
        }),
    ] {
        let (ctx, paths) = world("refused");
        let _running = a_backup_running(&paths);
        let (status, body) = asked(ctx, caller.clone()).await;
        assert_ne!(status, 200, "{caller:?}: {body}");
        assert!(!body.to_string().contains("sonarr"), "{caller:?}: {body}");
    }
}

/// What `listening` hears next, as the payload it carries, or nothing within a moment.
async fn heard(listening: &mut Listening) -> Option<Value> {
    let said = tokio::time::timeout(Duration::from_millis(50), listening.next())
        .await
        .ok()
        .flatten()?;
    let data: String = said
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .collect();
    serde_json::from_str(&data).ok()
}

#[tokio::test]
async fn the_operator_hears_the_copy_whole_a_key_as_its_read_and_nobody_else_at_all() {
    let (ctx, paths) = world("streams");
    let _running = an_existing_setup_copying(&paths);
    let ctx = Arc::new(ctx);
    let live = Live::opening(Stopped::at(0));
    let mut operator = live
        .listening(None)
        .await
        .heard_as(Audience::of(&Caller::Operator));
    let mut keyed = live
        .listening(None)
        .await
        .heard_as(Audience::of(&key(Scope::Read)));
    let mut nobody = live.listening(None).await;
    let mut a_members_key =
        live.listening(None)
            .await
            .heard_as(Audience::of(&key(Scope::Member {
                id: MEMBER.to_owned(),
                name: "ana".to_owned(),
            })));

    let said = Dashboard::against(Arc::clone(&ctx)).gather(true).await;
    let copying: Vec<_> = said
        .into_iter()
        .filter(|said| said.said().contains(r#""kind":"copying""#))
        .collect();
    assert_eq!(
        copying.iter().map(Rendered::audience).collect::<Vec<_>>(),
        [Audience::Operator, Audience::Key]
    );
    for said in copying {
        live.say(said).await;
    }

    let whole = heard(&mut operator).await;
    assert_eq!(
        whole
            .as_ref()
            .and_then(|said| said.pointer("/data/running/scope/project")),
        Some(&json!("media"))
    );
    assert_eq!(heard(&mut operator).await, None);

    let keys = heard(&mut keyed).await;
    let Some(running) = keys.as_ref().and_then(|said| said.pointer("/data/running")) else {
        unreachable!("a key hears the copy running");
    };
    assert_eq!(running.get("scope"), Some(&json!({ "scope": "existing" })));
    assert_eq!(running.get("job"), None, "{running}");
    assert!(!running.to_string().contains("/srv"), "{running}");
    assert_eq!(heard(&mut keyed).await, None);

    assert_eq!(heard(&mut nobody).await, None);
    assert_eq!(heard(&mut a_members_key).await, None);

    assert_eq!(EventKind::Copying.audience(), Audience::OperatorAndKeys);
    assert_eq!(EventKind::Copying.replayed(), Replayed::Never);
}

#[tokio::test]
async fn a_members_stream_never_carries_the_copy() {
    let (ctx, paths) = world("theirs");
    let _running = a_backup_running(&paths);
    let ctx = Arc::new(ctx);
    let caller = Caller::Member(MEMBER.to_owned());
    let Some(theirs) = Theirs::for_member(Arc::clone(&ctx), &caller) else {
        unreachable!("a member is handed a stream of their own");
    };
    let heard = theirs.gather(true).await;
    assert!(!heard.is_empty(), "the member's stream said nothing at all");
    assert!(
        heard
            .iter()
            .all(|said| !said.said().contains(r#""kind":"copying""#)),
        "a member heard the copy"
    );
}
