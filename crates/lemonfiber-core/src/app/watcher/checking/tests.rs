use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::scratch::Scratch;

use super::super::covering::{EXPIRING, GUARD};
use super::super::fixtures::{bare, installed};
use super::super::kept::{self, Kept, Made};
use super::{checked_in, CheckIn};
use crate::app::Ctx;
use crate::ports::service::{MonitorKind, Provision};

/// Where a job checks in.
const CHECK_IN: &str = "/lemonfiber/status.watch/v1/check_in";

/// A record holding the guard's heartbeat.
fn guarded(ctx: &Ctx) {
    let tag = GUARD.covers().tag();
    let mut record = Kept::default();
    record.made.insert(
        tag.clone(),
        Made {
            id: "9".to_owned(),
            asked: Provision {
                tag,
                kind: MonitorKind::Heartbeat,
                target: GUARD.id.to_owned(),
                every_s: GUARD.every_s,
                grace_s: Some(GUARD.grace_s),
                retries: 0,
            },
            path: None,
            unconfigured: None,
        },
    );
    kept::keep(ctx, &record);
}

/// How many check-ins the watcher was told of.
fn told(http: &Fake) -> usize {
    http.requests()
        .iter()
        .filter(|one| one.url.ends_with(CHECK_IN))
        .count()
}

#[tokio::test]
async fn a_job_with_a_heartbeat_checks_in_and_hears_whether_it_was_taken() {
    for (named, answer, came_to) in [
        ("taken", Answer::reply(204, ""), CheckIn::Taken),
        ("missed", Answer::reply(503, ""), CheckIn::Missed),
    ] {
        let project = Scratch::new(&format!("watcher-check-in-{named}"));
        let http = Fake::by_path(vec![(CHECK_IN, answer)]);
        let ctx = installed(&project, true, http.clone());
        guarded(&ctx);

        assert_eq!(checked_in(&ctx, GUARD).await, came_to, "{named}");
        assert_eq!(told(&http), 1, "{named}");
    }
}

#[tokio::test]
async fn nothing_is_checked_in_where_there_is_nothing_to_check_in_to() {
    let project = Scratch::new("watcher-check-in-unheld");
    let http = Fake::by_path(vec![(CHECK_IN, Answer::reply(204, ""))]);
    let ctx = installed(&project, true, http.clone());
    guarded(&ctx);
    assert_eq!(checked_in(&ctx, EXPIRING).await, CheckIn::Unwatched);

    let mut rehearsing = installed(&project, true, http.clone());
    rehearsing.dry_run = true;
    assert_eq!(checked_in(&rehearsing, GUARD).await, CheckIn::Unwatched);

    let unwatched = bare(
        &Scratch::new("watcher-check-in-unwatched"),
        true,
        http.clone(),
    );
    guarded(&unwatched);
    assert_eq!(checked_in(&unwatched, GUARD).await, CheckIn::Unwatched);

    let mut stackless = installed(
        &Scratch::new("watcher-check-in-stackless"),
        true,
        http.clone(),
    );
    guarded(&stackless);
    stackless.stack = crate::test_support::nowhere();
    assert_eq!(checked_in(&stackless, GUARD).await, CheckIn::Unwatched);

    assert_eq!(told(&http), 0);
}
