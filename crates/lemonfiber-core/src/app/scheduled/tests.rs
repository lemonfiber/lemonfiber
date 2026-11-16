use std::sync::Arc;
use std::time::Duration;

use lemonfiber_fixtures::ports::{Stopped, Ticking};
use lemonfiber_fixtures::scratch::Scratch;

use super::{kept_for, owe, records, run, standing, started, DOWN, RUNNING, TIMELESS};
use crate::app::Ctx;
use crate::error::codes::host::NOT_SCHEDULED;
use crate::instant::MINUTE;
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::process::Output;
use crate::scheduled::fixtures::{installed, SETTING};
use crate::scheduled::kept::{self, Kept};
use crate::stack::compose::Action;
use crate::test_support::{spoke, Recording, Reporting};

/// A context over `scratch` holding one plugin whose `configarr` runs at `when`, its
/// engine holding a running container where `up`, its runs answered by `answer`.
fn holding(scratch: &Scratch, when: &str, up: bool, answer: Output) -> (Ctx, Arc<Recording>) {
    let config = scratch.path().join("config");
    let _ = std::fs::create_dir_all(&config);
    let register = serde_json::json!({ "installed": installed(&[("configarr", Some(when))]) });
    let _ = std::fs::write(
        config.join(crate::config::paths::PLUGINS),
        register.to_string(),
    );
    let runner = Arc::new(Recording::answering(Ok(answer)));
    let engine = if up {
        Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy)
    } else {
        Reporting::holding(&[], Lifecycle::Running, Health::Healthy)
    };
    let ctx = crate::test_support::a_context()
        .runner(runner.clone())
        .engine(Arc::new(engine))
        .clock(Stopped::today())
        .settings(crate::config::Settings {
            env_file: Some(config.join(".env")),
            stack_dir: Some(scratch.path().join("data").join("stack")),
            ..crate::config::Settings::default()
        })
        .build();
    crate::test_support::every_part_has_run(&ctx);
    (ctx, runner)
}

fn exited(code: i32, said: &str) -> Output {
    Output {
        status: Some(code),
        stdout: said.to_owned(),
        stderr: String::new(),
    }
}

fn directory(ctx: &Ctx) -> std::path::PathBuf {
    records(ctx).unwrap_or_else(|| unreachable!("configured"))
}

fn kept_as(ctx: &Ctx, kept: &Kept) {
    kept::write(&directory(ctx), "configarr", kept)
        .unwrap_or_else(|why| unreachable!("written: {why:?}"));
}

fn ran_once(runner: &Recording) -> bool {
    runner
        .seen()
        .iter()
        .any(|argv| argv.iter().any(|word| word == "--no-deps"))
}

fn raised(ctx: &Ctx) -> Option<crate::condition::Condition> {
    crate::app::conditions::load(ctx)
        .get("services.scheduled.configarr")
        .filter(|condition| condition.is_raised())
        .cloned()
}

#[tokio::test]
async fn a_service_with_no_schedule_is_refused() {
    let scratch = Scratch::new("unscheduled");
    let (ctx, runner) = holding(&scratch, "@daily", true, spoke(""));
    let refused = run(&ctx, "jellyfin")
        .await
        .err()
        .unwrap_or_else(|| unreachable!("refused"));
    assert_eq!(refused.code, NOT_SCHEDULED);
    assert!(!ran_once(&runner));
}

#[tokio::test]
async fn an_owed_run_runs_to_completion_and_is_recorded() {
    let scratch = Scratch::new("owed");
    let (ctx, runner) = holding(&scratch, "0 0 1 1 *", true, spoke("synced"));
    owe(&ctx, &installed(&[("configarr", Some("@daily"))]), None);
    let report = run(&ctx, "configarr")
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    assert!(report.due, "{report:?}");
    assert!(ran_once(&runner), "{:?}", runner.seen());
    let ran = report.ran.unwrap_or_else(|| unreachable!("ran"));
    assert_eq!(ran.exit, Some(0));
    assert_eq!(ran.log, vec!["synced".to_owned()]);
    let kept = kept_for(&ctx, "configarr");
    assert!(!kept.owed);
    assert_eq!(
        kept.considered,
        Some(crate::scheduled::fires::minute_of(ran.ended))
    );
    assert_eq!(kept.last, Some(ran));
    assert!(raised(&ctx).is_none());
}

#[tokio::test]
async fn a_minute_none_of_its_times_fell_in_runs_nothing_and_records_nothing() {
    let scratch = Scratch::new("not-due");
    let (ctx, runner) = holding(&scratch, "* * * * *", true, spoke(""));
    let considered = Kept {
        considered: Some(crate::scheduled::fires::minute_of(ctx.stamp())),
        ..Kept::default()
    };
    kept_as(&ctx, &considered);
    let report = run(&ctx, "configarr")
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    assert!(!report.due);
    assert!(report.ran.is_none());
    assert!(!ran_once(&runner));
    assert_eq!(kept_for(&ctx, "configarr"), considered);
}

#[tokio::test]
async fn a_time_missed_since_the_last_run_runs_it_once() {
    let scratch = Scratch::new("missed");
    let (ctx, runner) = holding(&scratch, "*/5 * * * *", true, spoke(""));
    kept_as(
        &ctx,
        &Kept {
            considered: Some(ctx.stamp().earlier(30 * MINUTE)),
            ..Kept::default()
        },
    );
    let report = run(&ctx, "configarr")
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    assert!(report.due);
    assert_eq!(runner.seen().len(), 1);
}

#[tokio::test]
async fn a_run_that_exits_non_zero_raises_its_exit_and_the_next_that_exits_zero_clears_it() {
    let scratch = Scratch::new("failed");
    let (ctx, _) = holding(
        &scratch,
        "* * * * *",
        true,
        exited(2, "one\ntwo\nrefused: 401"),
    );
    owe(&ctx, &installed(&[("configarr", Some("@daily"))]), None);
    let report = run(&ctx, "configarr")
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    assert_eq!(report.ran.as_ref().and_then(|ran| ran.exit), Some(2));
    let condition = raised(&ctx).unwrap_or_else(|| unreachable!("raised"));
    assert_eq!(condition.exit, Some(2));
    assert_eq!(
        condition.kind,
        crate::error::codes::host::SCHEDULED_RUN_FAILED.as_str()
    );
    assert!(condition.summary.contains("exited with 2"), "{condition:?}");
    assert!(!condition.meaning.contains("refused: 401"), "{condition:?}");

    let (ctx, _) = holding(&scratch, "* * * * *", true, spoke("fine"));
    owe(&ctx, &installed(&[("configarr", Some("@daily"))]), None);
    run(&ctx, "configarr")
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    assert!(raised(&ctx).is_none());
}

#[tokio::test]
async fn a_stack_that_is_down_is_not_looked_at() {
    let scratch = Scratch::new("down");
    let (ctx, runner) = holding(&scratch, "* * * * *", false, spoke(""));
    owe(&ctx, &installed(&[("configarr", Some("@daily"))]), None);
    let report = run(&ctx, "configarr")
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    assert_eq!(report.held.as_deref(), Some(DOWN));
    assert!(!ran_once(&runner));
    assert!(kept_for(&ctx, "configarr").owed);
}

#[tokio::test]
async fn a_second_run_while_one_holds_the_lock_looks_at_nothing() {
    let scratch = Scratch::new("locked");
    let (ctx, runner) = holding(&scratch, "* * * * *", true, spoke(""));
    let lock = kept::locked(&directory(&ctx), "configarr")
        .unwrap_or_else(|why| unreachable!("opened: {why:?}"));
    let report = run(&ctx, "configarr")
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    drop(lock);
    assert_eq!(report.held.as_deref(), Some(RUNNING));
    assert!(!ran_once(&runner));
}

#[tokio::test]
async fn times_falling_due_while_it_runs_are_skipped_and_recorded() {
    let scratch = Scratch::new("skipped");
    let (mut ctx, _) = holding(&scratch, "* * * * *", true, spoke(""));
    let clock = Arc::new(Ticking::by(Duration::from_secs(5 * MINUTE)));
    ctx.seams.clock = clock;
    owe(&ctx, &installed(&[("configarr", Some("@daily"))]), None);
    let report = run(&ctx, "configarr")
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    assert!(report.skipped > 0, "{report:?}");
    let kept = kept_for(&ctx, "configarr");
    assert_eq!(kept.skipped, report.skipped);
    assert!(kept.last_skipped.is_some());
}

#[tokio::test]
async fn a_rehearsal_says_it_is_due_and_runs_nothing() {
    let scratch = Scratch::new("rehearsed");
    let (mut ctx, runner) = holding(&scratch, "* * * * *", true, spoke(""));
    owe(&ctx, &installed(&[("configarr", Some("@daily"))]), None);
    ctx.dry_run = true;
    let report = run(&ctx, "configarr")
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    assert!(report.due && report.rehearsed && report.ran.is_none());
    assert!(!ran_once(&runner));
    assert!(kept_for(&ctx, "configarr").owed);
}

fn carried(status: Option<i32>, rehearsed: bool) -> crate::model::LifecycleReport {
    let mut report = crate::app::boot::nothing_started("");
    report.status = status;
    report.rehearsed = rehearsed;
    report
}

#[tokio::test]
async fn a_start_or_restart_carried_out_owes_the_services_it_reached_one_run() {
    let scratch = Scratch::new("started");
    let (ctx, _) = holding(&scratch, "@daily", true, spoke(""));
    let owed = |ctx: &Ctx| kept_for(ctx, "configarr").owed;

    started(&ctx, &Action::Up, &carried(Some(0), true));
    started(&ctx, &Action::Up, &carried(Some(1), false));
    started(&ctx, &Action::Down, &carried(Some(0), false));
    started(
        &ctx,
        &Action::Restart(vec!["jellyfin".to_owned()]),
        &carried(Some(0), false),
    );
    assert!(!owed(&ctx));
    started(
        &ctx,
        &Action::Restart(vec!["configarr".to_owned()]),
        &carried(Some(0), false),
    );
    assert!(owed(&ctx));

    kept_as(&ctx, &Kept::default());
    started(&ctx, &Action::Up, &carried(Some(0), false));
    assert!(owed(&ctx));
}

#[tokio::test]
async fn a_restart_of_a_run_and_exit_service_owes_it_a_run_rather_than_starting_it() {
    let scratch = Scratch::new("restarted");
    let (ctx, runner) = holding(&scratch, "@daily", true, spoke(""));
    let said = crate::settings::restarted(&ctx, &["configarr".to_owned()]).await;
    assert!(said.is_none(), "{said:?}");
    assert!(runner.seen().is_empty(), "{:?}", runner.seen());
    assert!(kept_for(&ctx, "configarr").owed);
}

#[tokio::test]
async fn a_lock_that_cannot_be_opened_refuses_the_run() {
    let scratch = Scratch::new("unlockable");
    let (ctx, runner) = holding(&scratch, "* * * * *", true, spoke(""));
    let _ = std::fs::write(directory(&ctx), "");
    assert!(run(&ctx, "configarr").await.is_err());
    assert!(!ran_once(&runner));
}

#[tokio::test]
async fn a_stack_stopped_on_purpose_is_not_looked_at() {
    let scratch = Scratch::new("halted");
    let (ctx, runner) = holding(&scratch, "* * * * *", true, spoke(""));
    let mut returning = crate::autostart::Returning::default();
    returning.stopped();
    crate::autostart::run::save(&ctx, &returning);
    owe(&ctx, &installed(&[("configarr", Some("@daily"))]), None);
    let report = run(&ctx, "configarr")
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    assert_eq!(report.held.as_deref(), Some(DOWN));
    assert!(!ran_once(&runner));
}

#[tokio::test]
async fn a_setting_naming_no_time_raises_it_and_runs_nothing_and_a_rehearsal_raises_nothing() {
    let scratch = Scratch::new("timeless");
    let (ctx, runner) = holding(&scratch, "* * * * *", true, spoke(""));
    let settings = scratch
        .path()
        .join("config")
        .join(crate::config::paths::SETTINGS);
    assert!(
        crate::settings::store::recorded(&settings, &[(SETTING, Some("never"))].into()).is_ok()
    );

    let rehearsed = run(&ctx.clone().rehearsing(), "configarr")
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    assert_eq!(rehearsed.held.as_deref(), Some(TIMELESS));
    assert!(raised(&ctx).is_none());

    let report = run(&ctx, "configarr")
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    assert_eq!(report.held.as_deref(), Some(TIMELESS));
    assert!(!ran_once(&runner));
    assert!(raised(&ctx).is_some());
}

#[tokio::test]
async fn a_run_that_could_not_be_started_ends_with_no_exit_and_says_why() {
    let scratch = Scratch::new("unstarted");
    let (mut ctx, _) = holding(&scratch, "* * * * *", true, spoke(""));
    let runner = Arc::new(Recording::answering(Err(
        crate::ports::process::Failure::NotFound {
            program: "docker".to_owned(),
        },
    )));
    ctx.seams.runner = runner.clone();
    owe(&ctx, &installed(&[("configarr", Some("@daily"))]), None);
    let report = run(&ctx, "configarr")
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    let ran = report.ran.unwrap_or_else(|| unreachable!("ran"));
    assert_eq!(ran.exit, None);
    assert!(
        ran.log.iter().any(|line| line.contains("docker")),
        "{:?}",
        ran.log
    );
    assert!(ran_once(&runner));
    assert!(raised(&ctx).is_some());
}

#[tokio::test]
async fn each_run_and_exit_service_stands_as_its_last_run_left_it() {
    use crate::ports::docker::Engine as _;
    let scratch = Scratch::new("standing");
    let (ctx, _) = holding(&scratch, "* * * * *", true, exited(1, "refused"));
    let scheduled = installed(&[("configarr", Some("@daily")), ("kept", None)]);
    owe(&ctx, &scheduled, None);
    let _ = run(&ctx, "configarr").await;
    let containers = Reporting::holding(&["configarr"], Lifecycle::Running, Health::Healthy)
        .list(&ctx.settings.project)
        .await
        .unwrap_or_default();

    let found = standing(&ctx, &scheduled, &containers).await;

    let [one] = found.as_slice() else {
        unreachable!("one run-and-exit service: {found:?}");
    };
    assert_eq!(one.id, "configarr");
    assert!(one.running);
    assert!(!one.fired);
    assert!(!one.healthy());
    assert_eq!(one.last.as_ref().and_then(|ran| ran.exit), Some(1));
}

#[tokio::test]
async fn a_rehearsal_owes_nothing_and_a_register_that_cannot_be_read_owes_nothing() {
    let scratch = Scratch::new("owed-nothing");
    let (ctx, _) = holding(&scratch, "@daily", true, spoke(""));
    owe(
        &ctx.clone().rehearsing(),
        &installed(&[("configarr", Some("@daily"))]),
        None,
    );
    assert!(!kept_for(&ctx, "configarr").owed);
    let _ = std::fs::write(
        scratch
            .path()
            .join("config")
            .join(crate::config::paths::PLUGINS),
        "{",
    );
    started(&ctx, &Action::Up, &carried(Some(0), false));
    assert!(!kept_for(&ctx, "configarr").owed);
}

#[tokio::test]
async fn a_run_with_nowhere_to_keep_its_record_is_refused() {
    let scratch = Scratch::new("nowhere");
    let (mut ctx, runner) = holding(&scratch, "@daily", true, spoke(""));
    ctx.settings.env_file = None;

    let refused = run(&ctx, "configarr")
        .await
        .err()
        .map(|problem| problem.code);

    assert_eq!(
        refused,
        Some(crate::error::Diagnose::problem(&crate::config::store::Failure::Nowhere).code)
    );
    assert!(!ran_once(&runner));
}

#[tokio::test]
async fn a_run_and_exit_service_whose_job_this_machine_fires_reads_fired() {
    use crate::ports::docker::Engine as _;
    use crate::ports::hosting::{Manager, Standing};
    use lemonfiber_fixtures::hosting::Hosting;
    let scratch = Scratch::new("fired");
    let (ctx, _) = holding(&scratch, "@daily", true, spoke(""));
    let job = crate::scheduled::job("configarr");
    let ctx = ctx.with_hosting(Hosting::holding(
        Manager::Launchd,
        &job,
        Hosting::installed(&job, Standing::Running),
    ) as Arc<dyn crate::ports::Host>);
    let scheduled = installed(&[("configarr", Some("@daily"))]);
    let containers = Reporting::holding(&["configarr"], Lifecycle::Exited, Health::None)
        .list(&ctx.settings.project)
        .await
        .unwrap_or_default();

    let found = standing(&ctx, &scheduled, &containers).await;

    assert!(found.iter().all(|one| one.fired), "{found:?}");
    assert_eq!(found.len(), 1);
}

#[tokio::test]
async fn a_run_and_exit_services_container_is_no_stranger_and_any_other_is() {
    let scratch = Scratch::new("strangers");
    let (mut ctx, _) = holding(&scratch, "@daily", true, spoke(""));
    ctx.seams.engine = Arc::new(Reporting::holding(
        &["configarr", "whoever"],
        Lifecycle::Exited,
        Health::None,
    ));

    let answered =
        crate::app::dispatch(crate::app::Command::Status { forms: Vec::new() }, &ctx).await;

    let Ok(crate::app::Outcome::Status(report)) = answered else {
        unreachable!("a status answers with a status");
    };
    let strangers: Vec<&str> = report
        .undeclared
        .iter()
        .map(|one| one.id.as_str())
        .collect();
    assert!(strangers.contains(&"whoever"), "{strangers:?}");
    assert!(!strangers.contains(&"configarr"), "{strangers:?}");
}
