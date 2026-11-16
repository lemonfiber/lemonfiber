use super::with_runs;
use crate::docker::Condition;
use crate::model::{RunHealth, ScheduledService};

fn service(health: RunHealth) -> ScheduledService {
    ScheduledService {
        id: "configarr".to_owned(),
        name: "Configarr".to_owned(),
        running: false,
        fired: true,
        health,
        last: None,
        skipped: 0,
    }
}

#[test]
fn a_running_stack_whose_scheduled_run_failed_reads_degraded_and_any_other_reads_as_it_was() {
    let failed = [service(RunHealth::Succeeded), service(RunHealth::Failed)];
    let fine = [service(RunHealth::NotRun), service(RunHealth::Succeeded)];
    assert_eq!(with_runs(Condition::Active, &failed), Condition::Degraded);
    assert_eq!(with_runs(Condition::Partial, &failed), Condition::Degraded);
    assert_eq!(with_runs(Condition::Inactive, &failed), Condition::Inactive);
    assert_eq!(with_runs(Condition::Active, &fine), Condition::Active);
}
