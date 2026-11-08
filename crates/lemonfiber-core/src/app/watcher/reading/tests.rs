use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::scratch::Scratch;

use super::super::fixtures::{bare, installed, PLUGIN};
use super::super::kept::{self, Kept, Made};
use super::watcher;
use crate::app::Ctx;
use crate::model::{Unconfigured, WatchPath, WatchState, WatcherReport};
use crate::ports::service::{Covers, MonitorKind, Provision};

/// Where the watcher is asked whether it answers.
const REACHABLE: &str = "/lemonfiber/status.watch/v1/reachable";

/// Where the watcher is asked every monitor it holds.
const MONITORS: &str = "/lemonfiber/status.watch/v1/monitors";

/// A watcher answering `reachable` and `monitors` with these.
fn answering(reachable: Answer, monitors: Answer) -> Arc<Fake> {
    Fake::by_path(vec![(REACHABLE, reachable), (MONITORS, monitors)])
}

/// A watcher that answers, holding `monitors`.
fn holding(monitors: &serde_json::Value) -> Arc<Fake> {
    answering(
        Answer::reply(200, r#"{"reachable":true,"why":null}"#),
        Answer::reply(200, monitors.to_string()),
    )
}

/// The read, or the empty report no assertion below is satisfied by.
async fn read(ctx: &Ctx) -> WatcherReport {
    watcher(ctx).await.unwrap_or_default()
}

/// A monitor as the watcher answers it.
fn monitor(id: &str, tag: Option<&str>, state: &str, last: Option<u64>) -> serde_json::Value {
    serde_json::json!({
        "id": id, "tag": tag, "name": format!("monitor {id}"), "kind": "http",
        "target": "http://somewhere", "active": true, "heartbeat": null, "state": state,
        "since": 1_000, "last_reading": last,
    })
}

/// A record of one monitor under `covers`, by `id`.
fn made(id: &str, covers: &Covers) -> (String, Made) {
    (
        covers.tag(),
        Made {
            id: id.to_owned(),
            asked: Provision {
                tag: covers.tag(),
                kind: MonitorKind::Http,
                target: "http://somewhere".to_owned(),
                every_s: 60,
                grace_s: None,
                retries: 3,
            },
            path: None,
            unconfigured: None,
        },
    )
}

/// The moment the fixed clock reads, in seconds since the Unix epoch.
fn now(ctx: &Ctx) -> u64 {
    ctx.seconds()
}

#[tokio::test]
async fn a_stack_nothing_watches_is_answered_blind_with_everything_it_should_cover_uncovered() {
    let project = Scratch::new("watcher-read-unwatched");
    let ctx = bare(&project, true, Fake::by_path(Vec::new()));

    let report = read(&ctx).await;

    assert_eq!(report.watcher, None);
    assert!(!report.reachable);
    assert_eq!(report.read_at, now(&ctx));
    assert_eq!(
        report.unreachable_because.as_deref(),
        Some("nothing fills status.watch")
    );
    assert!(report.monitors.is_empty());
    assert!(report
        .uncovered
        .contains(&Covers::Service("sonarr".to_owned())));
    assert!(report
        .uncovered
        .iter()
        .any(|covers| matches!(covers, Covers::EntryPoint(_))));
    assert!(!report
        .uncovered
        .iter()
        .any(|covers| matches!(covers, Covers::Job(_))));
}

#[tokio::test]
async fn a_watcher_that_does_not_answer_is_answered_blind_and_says_why() {
    let cases = [
        (
            "unkeyed",
            false,
            answering(Answer::reply(200, "{}"), Answer::reply(200, "[]")),
            format!("{PLUGIN} cannot be asked over status.watch"),
        ),
        (
            "locked",
            true,
            answering(
                Answer::reply(200, r#"{"reachable":false,"why":"its database is locked"}"#),
                Answer::reply(200, "[]"),
            ),
            "its database is locked".to_owned(),
        ),
        (
            "unsaid",
            true,
            answering(
                Answer::reply(200, r#"{"reachable":false,"why":null}"#),
                Answer::reply(200, "[]"),
            ),
            "it says it does not answer".to_owned(),
        ),
        (
            "down",
            true,
            answering(Answer::reply(503, ""), Answer::reply(200, "[]")),
            "503 is not an answer the contract declares".to_owned(),
        ),
        (
            "unlisted",
            true,
            answering(
                Answer::reply(200, r#"{"reachable":true,"why":null}"#),
                Answer::reply(503, ""),
            ),
            "503 is not an answer the contract declares".to_owned(),
        ),
    ];
    for (named, keyed, http, why) in cases {
        let project = Scratch::new(&format!("watcher-read-{named}"));
        let ctx = installed(&project, keyed, http);

        let report = read(&ctx).await;

        assert_eq!(report.watcher.as_deref(), Some(PLUGIN), "{named}");
        assert!(!report.reachable, "{named}");
        assert!(
            report
                .unreachable_because
                .as_deref()
                .is_some_and(|said| said.contains(&why)),
            "{named}: {:?}",
            report.unreachable_because
        );
        assert!(report.monitors.is_empty(), "{named}");
        assert!(!report.uncovered.is_empty(), "{named}");
    }
}

#[tokio::test]
async fn each_monitor_is_paired_by_the_record_alone_and_said_as_it_stands() {
    let project = Scratch::new("watcher-read-paired");
    let probe = installed(&project, true, Fake::by_path(Vec::new()));
    let at = now(&probe);
    let sonarr = Covers::Service("sonarr".to_owned());
    let radarr = Covers::Service("radarr".to_owned());
    let seerr = Covers::EntryPoint("seerr".to_owned());
    let guard = Covers::Job("guard".to_owned());
    let beat = serde_json::json!({
        "id": "4", "tag": guard.tag(), "name": "guard", "kind": "heartbeat",
        "target": "guard", "active": true,
        "heartbeat": { "every_s": 60, "grace_s": 60, "last_check_in": at - 30 },
        "state": "up", "since": at - 30, "last_reading": at - 30,
    });
    let mut paused = monitor("5", None, "paused", None);
    if let Some(active) = paused.get_mut("active") {
        *active = serde_json::json!(false);
    }
    let monitors = serde_json::json!([
        monitor("1", Some("renamed"), "up", Some(at - 10)),
        monitor("2", None, "down", Some(at - 500)),
        monitor("3", None, "pending", Some(at - 10)),
        beat,
        paused,
        monitor("6", Some(&radarr.tag()), "up", Some(at - 10)),
    ]);
    let ctx = installed(&project, true, holding(&monitors));
    let mut record = Kept::default();
    record.made.extend([
        made("1", &sonarr),
        made("2", &seerr),
        made("3", &radarr),
        made("4", &guard),
    ]);
    if let Some(entry) = record.made.get_mut(&seerr.tag()) {
        entry.path = Some(WatchPath::Internal);
        entry.unconfigured = Some(Unconfigured {
            wanted: "the address the household is handed".to_owned(),
            watches: "http://somewhere".to_owned(),
            why: "no address".to_owned(),
        });
    }
    kept::keep(&ctx, &record);

    let report = read(&ctx).await;

    assert!(report.reachable);
    assert_eq!(report.unreachable_because, None);
    let seen: Vec<(Option<Covers>, WatchState)> = report
        .monitors
        .iter()
        .map(|one| (one.covers.clone(), one.state))
        .collect();
    assert_eq!(
        seen,
        [
            (Some(sonarr.clone()), WatchState::Up),
            (Some(seerr.clone()), WatchState::NotReporting),
            (Some(radarr.clone()), WatchState::Pending),
            (Some(guard), WatchState::Up),
            (None, WatchState::Paused),
            (None, WatchState::Up),
        ]
    );
    let one = |index: usize| report.monitors.get(index);
    assert!(
        one(1).is_some_and(|entry| entry.path == Some(WatchPath::Internal)
            && entry
                .unconfigured
                .as_ref()
                .is_some_and(|said| said.why == "no address"))
    );
    assert!(one(3)
        .and_then(|guarded| guarded.heartbeat)
        .is_some_and(|beat| beat.every_s == 60 && beat.last_check_in == Some(at - 30)));
    assert!(one(4).is_some_and(|paused| !paused.active));
    assert!(one(0).is_some_and(|first| first.name == "monitor 1"
        && first.kind == MonitorKind::Http
        && first.since == 1_000
        && first.last_reading == Some(at - 10)));
    assert!(!report.uncovered.contains(&sonarr));
    assert!(!report.uncovered.contains(&seerr));
    assert!(!report.uncovered.contains(&radarr));
    assert!(report
        .uncovered
        .contains(&Covers::Service("lidarr".to_owned())));
}

#[tokio::test]
async fn what_the_stack_was_stopped_on_purpose_under_is_expected_down() {
    let project = Scratch::new("watcher-read-stopped");
    let sonarr = Covers::Service("sonarr".to_owned());
    let guard = Covers::Job("guard".to_owned());
    let monitors = serde_json::json!([
        monitor("1", None, "down", Some(1_000)),
        monitor("2", None, "down", Some(1_000)),
    ]);
    let ctx = installed(&project, true, holding(&monitors));
    let mut record = Kept::default();
    record.made.extend([made("1", &sonarr), made("2", &guard)]);
    kept::keep(&ctx, &record);
    let mut returning = crate::autostart::run::load(&ctx);
    returning.stopped();
    crate::autostart::run::save(&ctx, &returning);

    let report = read(&ctx).await;

    let states: Vec<WatchState> = report.monitors.iter().map(|one| one.state).collect();
    assert_eq!(states, [WatchState::ExpectedDown, WatchState::NotReporting]);
}

#[tokio::test]
async fn a_record_or_a_stack_that_cannot_be_read_is_refused_never_answered_as_nothing_watched() {
    let project = Scratch::new("watcher-read-damaged");
    let ctx = installed(&project, true, holding(&serde_json::json!([])));
    let record =
        crate::app::targets::beside_env(&ctx, crate::config::paths::WATCHER).unwrap_or_default();
    let _ = std::fs::write(&record, "not a record");
    assert!(watcher(&ctx).await.is_err());

    let mut stackless = installed(
        &Scratch::new("watcher-read-stackless"),
        true,
        holding(&serde_json::json!([])),
    );
    stackless.stack = crate::test_support::nowhere();
    assert!(watcher(&stackless).await.is_err());
}
