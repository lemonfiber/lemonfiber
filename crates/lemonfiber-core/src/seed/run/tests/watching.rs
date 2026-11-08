//! The outside watcher, held over `status.watch` to a monitor for every critical service,
//! household entry point and hosted job, each under lemonfiber's own tag.

use super::super::watching::{remonitor, remonitored, seed_watcher};
use super::*;
use crate::app::watcher::kept::{self, Kept, Made};
use crate::ports::hosting::{Manager, Standing};
use crate::ports::service::{Covers, MonitorKind, Provision};
use lemonfiber_fixtures::hosting::Hosting;

/// The plugin a contract-speaking watcher is installed as.
const WATCHER: &str = "watcher";

/// [`WATCHER`] as first-party.
const WATCHER_TRUSTED: [crate::plugin::first_party::FirstParty; 1] =
    crate::test_support::first_party(WATCHER);

/// Where the watcher is asked every monitor it holds.
const MONITORS: &str = "/lemonfiber/status.watch/v1/monitors";

/// Where the watcher is told to make or update a monitor.
const PROVISION: &str = "/lemonfiber/status.watch/v1/provision";

/// Where the watcher is told to remove a monitor.
const RETIRE: &str = "/lemonfiber/status.watch/v1/retire";

/// What the watcher answers every provision with.
const PROVISIONED: &str = r#"{"id":"7","instead":null}"#;

/// The stack this build carries.
fn carried() -> Vec<lemonfiber_manifest::Service> {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.services)
        .unwrap_or_default()
}

/// The watcher's plugin beside the carried stack.
fn beside(project: &std::path::Path) -> crate::wiring::Fillers {
    let plugin = crate::test_support::contracted(WATCHER, WATCHER, "status.watch");
    fillers_trusting(carried(), &[plugin], project, &WATCHER_TRUSTED)
}

/// A context reaching the watcher over `http`, on a machine whose service manager hosts
/// the guard and nothing else.
fn watching(project: &std::path::Path, http: Arc<Fake>) -> Ctx {
    let manager = Hosting::holding(
        Manager::Launchd,
        "watch",
        Hosting::installed("watch", Standing::Running),
    );
    contracted_ctx(project, WATCHER, true, http)
        .with_hosting(manager as Arc<dyn crate::ports::Host>)
}

/// What the step settled in over [`beside`], where it ran.
async fn seeded(ctx: &Ctx, project: &std::path::Path) -> Option<State> {
    seed_watcher(ctx, &carried(), &[], &beside(project))
        .await
        .map(|wiring| wiring.state)
}

/// The tag of every provision the watcher was told, in order.
fn provisioned(http: &Fake) -> Vec<String> {
    http.requests()
        .iter()
        .filter(|one| one.url.ends_with(PROVISION))
        .filter_map(|one| one.body.as_deref())
        .filter_map(|body| serde_json::from_str::<serde_json::Value>(body).ok())
        .filter_map(|body| {
            body.pointer("/provision/tag")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

/// The tag of every monitor the watcher was told to remove.
fn retired(http: &Fake) -> Vec<String> {
    http.requests()
        .iter()
        .filter(|one| one.url.ends_with(RETIRE))
        .filter_map(|one| one.body.as_deref())
        .filter_map(|body| serde_json::from_str::<serde_json::Value>(body).ok())
        .filter_map(|body| {
            body.get("tag")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

/// A watcher holding `monitors`, taking every provision and retirement.
fn holding(monitors: &str) -> Arc<Fake> {
    Fake::by_path(vec![
        (MONITORS, Answer::reply(200, monitors.to_owned())),
        (PROVISION, Answer::reply(200, PROVISIONED)),
        (RETIRE, Answer::reply(204, "")),
    ])
}

/// A record holding one monitor under `tag`, by the id the watcher answers.
fn kept_as(tag: &str) -> Made {
    Made {
        id: "7".to_owned(),
        asked: Provision {
            tag: tag.to_owned(),
            kind: MonitorKind::Http,
            target: "http://gone:1".to_owned(),
            every_s: 60,
            grace_s: None,
            retries: 3,
        },
        path: None,
        unconfigured: None,
    }
}

#[tokio::test]
async fn every_critical_service_entry_point_and_hosted_job_is_given_a_monitor_under_its_tag() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("watcher-provisioned");
    let http = holding("[]");
    let ctx = watching(&project, http.clone());

    assert_eq!(seeded(&ctx, &project).await, Some(State::Wired));

    let tags = provisioned(&http);
    let critical: Vec<String> = carried()
        .iter()
        .filter(|service| {
            matches!(
                service.criticality,
                lemonfiber_manifest::Criticality::Critical | lemonfiber_manifest::Criticality::Core
            )
        })
        .map(|service| Covers::Service(service.id.clone()).tag())
        .collect();
    assert!(!critical.is_empty());
    for tag in &critical {
        assert!(tags.contains(tag), "{tag} in {tags:?}");
    }
    assert!(
        tags.iter().any(|tag| tag.starts_with("lemonfiber:entry:")),
        "{tags:?}"
    );
    assert!(
        tags.contains(&Covers::Job("guard".to_owned()).tag()),
        "{tags:?}"
    );
    assert!(
        !tags.contains(&Covers::Job("expiring".to_owned()).tag()),
        "{tags:?}"
    );
    assert!(
        tags.iter().all(|tag| Covers::read(tag).is_some()),
        "{tags:?}"
    );
    let record = kept::held(&ctx);
    assert_eq!(record.made.len(), tags.len());
    assert!(record.made.values().all(|made| made.id == "7"));
    assert!(retired(&http).is_empty());
}

#[tokio::test]
async fn a_watcher_already_holding_every_monitor_as_provisioned_is_left_as_it_is() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("watcher-held");
    let first = holding("[]");
    let ctx = watching(&project, first);
    assert_eq!(seeded(&ctx, &project).await, Some(State::Wired));

    let held = serde_json::json!([{
        "id": "7", "tag": null, "name": "made", "kind": "http", "target": "x",
        "active": true, "heartbeat": null, "state": "up", "since": 1, "last_reading": 1,
    }])
    .to_string();
    let again = holding(&held);
    let ctx = watching(&project, again.clone());
    assert_eq!(seeded(&ctx, &project).await, Some(State::AlreadyWired));

    let mut rehearsing = watching(&project, holding(&held));
    rehearsing.dry_run = true;
    assert_eq!(
        seeded(&rehearsing, &project).await,
        Some(State::AlreadyWired)
    );
}

#[tokio::test]
async fn a_rehearsal_says_how_many_it_would_hold_and_provisions_and_retires_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("watcher-rehearsed");
    let http = holding(
        r#"[{"id":"3","tag":"lemonfiber:job:old","name":"old","kind":"heartbeat","target":"old","active":true,"heartbeat":null,"state":"up","since":1,"last_reading":null}]"#,
    );
    let mut ctx = watching(&project, http.clone());
    ctx.dry_run = true;

    let state = seeded(&ctx, &project).await;

    assert!(
        matches!(&state, Some(State::WouldWire { yours: Some(yours), ours: Some(ours) })
            if yours == "0 monitors" && ours.ends_with("monitors")),
        "{state:?}"
    );
    assert!(provisioned(&http).is_empty());
    assert!(retired(&http).is_empty());
    assert_eq!(kept::held(&ctx), Kept::default());
}

#[tokio::test]
async fn a_tagged_monitor_covering_what_is_gone_is_retired_and_the_operators_are_not() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("watcher-retired");
    let monitors = serde_json::json!([
        { "id": "3", "tag": "lemonfiber:job:old", "name": "old", "kind": "heartbeat",
          "target": "old", "active": true, "heartbeat": null, "state": "up", "since": 1,
          "last_reading": null },
        { "id": "4", "tag": null, "name": "router", "kind": "ping", "target": "10.0.0.1",
          "active": true, "heartbeat": null, "state": "up", "since": 1, "last_reading": 1 },
        { "id": "5", "tag": "kuma:mine", "name": "mine", "kind": "dns", "target": "a",
          "active": true, "heartbeat": null, "state": "up", "since": 1, "last_reading": 1 },
    ])
    .to_string();
    let http = holding(&monitors);
    let ctx = watching(&project, http.clone());
    let gone = Covers::Service("gone".to_owned()).tag();
    let mut record = Kept::default();
    record.made.insert(gone.clone(), kept_as(&gone));
    kept::keep(&ctx, &record);

    assert_eq!(seeded(&ctx, &project).await, Some(State::Wired));

    assert_eq!(
        retired(&http),
        ["lemonfiber:job:old".to_owned(), gone.clone()]
    );
    assert!(!kept::held(&ctx).made.contains_key(&gone));
}

#[tokio::test]
async fn a_watcher_that_will_not_provision_or_retire_keeps_what_the_record_held() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("watcher-refusing");
    let http = Fake::by_path(vec![
        (MONITORS, Answer::reply(200, "[]")),
        (PROVISION, Answer::reply(503, "")),
        (RETIRE, Answer::reply(503, "")),
    ]);
    let ctx = watching(&project, http.clone());
    let guard = Covers::Job("guard".to_owned()).tag();
    let gone = Covers::Service("gone".to_owned()).tag();
    let mut record = Kept::default();
    record.made.insert(guard.clone(), kept_as(&guard));
    record.made.insert(gone.clone(), kept_as(&gone));
    kept::keep(&ctx, &record);

    let state = seeded(&ctx, &project).await;

    assert!(matches!(state, Some(State::Failed { .. })), "{state:?}");
    assert_eq!(kept::held(&ctx), record);
}

#[tokio::test]
async fn a_watcher_that_cannot_say_what_it_holds_is_told_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("watcher-unreached");
    let http = Fake::by_path(vec![(MONITORS, Answer::reply(503, ""))]);
    let ctx = watching(&project, http.clone());

    let state = seeded(&ctx, &project).await;

    assert!(matches!(state, Some(State::Failed { .. })), "{state:?}");
    assert!(provisioned(&http).is_empty());
}

#[tokio::test]
async fn a_stack_nothing_watches_is_not_asked() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("watcher-absent");
    let http = Fake::by_path(Vec::new());
    let ctx = watching(&project, http.clone());

    assert!(
        seed_watcher(&ctx, &carried(), &[], &fillers_at(carried(), &project))
            .await
            .is_none()
    );
    assert!(http.requests().is_empty());
}

/// A context with the watcher installed and filling `status.watch`, reached over `http`.
fn installed(project: &std::path::Path, http: Arc<Fake>) -> Ctx {
    crate::app::watcher::fixtures::installed(project, true, http)
}

#[tokio::test]
async fn the_watcher_is_provisioned_again_over_the_stack_and_what_is_installed() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("watcher-remonitored");
    let ctx = installed(&project, holding("[]"));
    assert!(remonitored(&ctx)
        .await
        .is_some_and(|said| said.starts_with("The outside watcher holds")));

    let held = serde_json::json!([{
        "id": "7", "tag": null, "name": "made", "kind": "http", "target": "x",
        "active": true, "heartbeat": null, "state": "up", "since": 1, "last_reading": 1,
    }])
    .to_string();
    let holding_every_one = holding(&held);
    assert_eq!(
        remonitored(&installed(&project, holding_every_one.clone())).await,
        None
    );
    assert!(!provisioned(&holding_every_one).is_empty());

    let refusing = installed(
        &project,
        Fake::by_path(vec![(MONITORS, Answer::reply(503, ""))]),
    );
    assert!(remonitored(&refusing)
        .await
        .is_some_and(|said| said.starts_with("The outside watcher's monitors could not")));

    let mut stackless = ctx;
    stackless.stack = crate::test_support::nowhere();
    assert_eq!(remonitor(&stackless).await, None);
}
