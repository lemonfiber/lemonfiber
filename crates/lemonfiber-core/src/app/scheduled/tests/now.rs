//! A run asked for at once: run whether or not a time fell due, under the fired run's
//! lock, held where a run is going or the stack is down, and recorded as a fired run.

use super::*;

fn considered_now(ctx: &Ctx) -> Kept {
    let considered = Kept {
        considered: Some(crate::scheduled::fires::minute_of(ctx.stamp())),
        ..Kept::default()
    };
    kept_as(ctx, &considered);
    considered
}

#[tokio::test]
async fn a_run_asked_for_now_runs_though_none_of_its_times_fell_due() {
    let scratch = Scratch::new("now");
    let (ctx, runner) = holding(&scratch, "0 0 1 1 *", true, spoke("synced"));
    considered_now(&ctx);
    let report = run(&ctx, "configarr", true)
        .await
        .unwrap_or_else(|why| unreachable!("ran: {why:?}"));
    assert!(report.due && !report.rehearsed, "{report:?}");
    assert!(report.held.is_none());
    assert_eq!(report.skipped, 0);
    assert!(ran_once(&runner), "{:?}", runner.seen());
    let ran = report.ran.unwrap_or_else(|| unreachable!("ran"));
    assert_eq!(ran.exit, Some(0));
    let kept = kept_for(&ctx, "configarr");
    assert_eq!(kept.last, Some(ran.clone()));
    assert_eq!(
        kept.considered,
        Some(crate::scheduled::fires::minute_of(ran.ended))
    );
    assert!(raised(&ctx).is_none());
}

#[tokio::test]
async fn a_run_asked_for_now_clears_an_owed_run_and_raises_a_failed_exit() {
    let scratch = Scratch::new("now-failed");
    let (ctx, _) = holding(&scratch, "0 0 1 1 *", true, exited(3, "refused"));
    owe(&ctx, &installed(&[("configarr", Some("@daily"))]), None);
    let report = run(&ctx, "configarr", true)
        .await
        .unwrap_or_else(|why| unreachable!("ran: {why:?}"));
    assert_eq!(report.ran.and_then(|ran| ran.exit), Some(3));
    assert!(!kept_for(&ctx, "configarr").owed);
    assert_eq!(raised(&ctx).and_then(|condition| condition.exit), Some(3));
}

#[tokio::test]
async fn a_run_asked_for_now_while_one_holds_the_lock_runs_nothing_and_says_why() {
    let scratch = Scratch::new("now-locked");
    let (ctx, runner) = holding(&scratch, "0 0 1 1 *", true, spoke(""));
    let considered = considered_now(&ctx);
    let lock = kept::locked(&directory(&ctx), "configarr")
        .unwrap_or_else(|why| unreachable!("opened: {why:?}"));
    let report = run(&ctx, "configarr", true)
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    drop(lock);
    assert_eq!(report.held.as_deref(), Some(RUNNING));
    assert!(report.ran.is_none());
    assert!(!ran_once(&runner));
    assert_eq!(kept_for(&ctx, "configarr"), considered);
}

#[tokio::test]
async fn a_run_asked_for_now_while_the_stack_is_down_runs_nothing_and_says_why() {
    let scratch = Scratch::new("now-down");
    let (ctx, runner) = holding(&scratch, "0 0 1 1 *", false, spoke(""));
    let considered = considered_now(&ctx);
    let report = run(&ctx, "configarr", true)
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    assert_eq!(report.held.as_deref(), Some(DOWN));
    assert!(report.ran.is_none());
    assert!(!ran_once(&runner));
    assert_eq!(kept_for(&ctx, "configarr"), considered);
}

#[tokio::test]
async fn a_rehearsed_run_asked_for_now_says_it_would_run_and_runs_nothing() {
    let scratch = Scratch::new("now-rehearsed");
    let (ctx, runner) = holding(&scratch, "0 0 1 1 *", true, spoke(""));
    let considered = considered_now(&ctx);
    let report = run(&ctx.clone().rehearsing(), "configarr", true)
        .await
        .unwrap_or_else(|why| unreachable!("looked at: {why:?}"));
    assert!(
        report.due && report.rehearsed && report.ran.is_none(),
        "{report:?}"
    );
    assert!(report.held.is_none());
    assert!(!ran_once(&runner));
    assert_eq!(kept_for(&ctx, "configarr"), considered);
}

#[tokio::test]
async fn times_falling_due_while_a_run_asked_for_now_runs_are_skipped_and_recorded() {
    let scratch = Scratch::new("now-skipped");
    let (mut ctx, _) = holding(&scratch, "* * * * *", true, spoke(""));
    ctx.seams.clock = Arc::new(Ticking::by(Duration::from_secs(5 * MINUTE)));
    let report = run(&ctx, "configarr", true)
        .await
        .unwrap_or_else(|why| unreachable!("ran: {why:?}"));
    assert!(report.skipped > 0, "{report:?}");
    let kept = kept_for(&ctx, "configarr");
    assert_eq!(kept.skipped, report.skipped);
    assert!(kept.last_skipped.is_some());
}

#[tokio::test]
async fn a_run_asked_for_now_of_a_service_whose_setting_names_no_time_runs_and_keeps_it_raised() {
    let scratch = Scratch::new("now-timeless");
    let (ctx, runner) = holding(&scratch, "* * * * *", true, spoke(""));
    let settings = scratch
        .path()
        .join("config")
        .join(crate::config::paths::SETTINGS);
    assert!(
        crate::settings::store::recorded(&settings, &[(SETTING, Some("never"))].into()).is_ok()
    );
    let report = run(&ctx, "configarr", true)
        .await
        .unwrap_or_else(|why| unreachable!("ran: {why:?}"));
    assert_eq!(report.ran.and_then(|ran| ran.exit), Some(0));
    assert_eq!(report.skipped, 0);
    assert!(ran_once(&runner));
    assert!(raised(&ctx).is_some());
}
