use super::fixtures::{installed, SETTING};
use super::{all, checked, named, reading};

#[test]
fn only_a_service_declaring_a_schedule_is_a_run_and_exit_one() {
    let installed = installed(&[("configarr", Some("@daily")), ("kept", None)]);
    let found = all(&installed);
    let [one] = found.as_slice() else {
        unreachable!("one scheduled: {found:?}");
    };
    assert_eq!(one.service, "configarr");
    assert_eq!(one.check(), "services.scheduled.configarr");
    assert!(named(&installed, "kept").is_none());
    assert!(named(&installed, "configarr").is_some());
}

#[test]
fn the_setting_names_the_times_and_the_default_stands_in_for_none() {
    let installed = installed(&[("configarr", Some("@daily"))]);
    let one = named(&installed, "configarr").unwrap_or_else(|| unreachable!("scheduled"));
    let daily = lemonfiber_manifest::schedule::Cron::parse("@daily");
    let hourly = lemonfiber_manifest::schedule::Cron::parse("0 * * * *");
    assert_eq!(one.times(&|_| None), daily);
    assert_eq!(one.times(&|_| Some("  ".to_owned())), daily);
    assert_eq!(one.times(&|_| Some("@hourly".to_owned())), hourly);
    assert_eq!(one.times(&|_| Some("whenever".to_owned())), None);
}

#[test]
fn a_setting_is_traced_to_the_service_whose_schedule_reads_it() {
    let found = all(&installed(&[("configarr", Some("@daily"))]));
    assert_eq!(
        reading(&found, SETTING).map(|one| one.service.as_str()),
        Some("configarr")
    );
    assert!(reading(&found, "configarr:other").is_none());
    assert_eq!(checked("x"), "services.scheduled.x");
}

fn ran(exit: Option<i32>, log: &[&str]) -> super::ScheduledRun {
    super::ScheduledRun {
        started: crate::instant::Instant::EPOCH,
        ended: crate::instant::Instant::EPOCH,
        exit,
        log: log.iter().map(|line| (*line).to_owned()).collect(),
    }
}

#[test]
fn a_run_that_did_not_exit_zero_says_how_and_its_last_lines_in_the_detail_alone() {
    assert!(super::failed("configarr", &ran(Some(0), &[])).is_none());
    let failed = super::failed("configarr", &ran(Some(3), &["one", "two"]))
        .unwrap_or_else(|| unreachable!("failed"));
    assert_eq!(failed.code, crate::error::codes::host::SCHEDULED_RUN_FAILED);
    assert!(failed.summary.ends_with("exited with 3"), "{failed:?}");
    assert!(!failed.meaning.contains("two"), "{failed:?}");
    assert_eq!(failed.detail.as_deref(), Some("one\ntwo"));
    let ended =
        super::failed("configarr", &ran(None, &[])).unwrap_or_else(|| unreachable!("failed"));
    assert!(
        ended.summary.ends_with("ended before it exited"),
        "{ended:?}"
    );
    assert!(ended.detail.is_none());

    let fault = super::fault(&failed, Some(3));
    assert_eq!(fault.exit, Some(3));
    assert_eq!(fault.kind, failed.code.as_str());
    assert_eq!(fault.summary, failed.summary);
}

#[test]
fn a_setting_naming_no_time_says_which() {
    let installed = installed(&[("configarr", Some("@daily"))]);
    let one = named(&installed, "configarr").unwrap_or_else(|| unreachable!("scheduled"));
    let timeless = super::timeless(&one);
    assert!(timeless.summary.contains(SETTING), "{timeless:?}");
    assert!(timeless.summary.starts_with("configarr"), "{timeless:?}");
}

#[test]
fn a_key_reads_a_failed_runs_finding_without_the_lines_it_wrote() {
    let problem = super::failed("configarr", &ran(Some(3), &["one", "two"]))
        .unwrap_or_else(|| unreachable!("failed"));
    let finding = |check: &str| {
        crate::doctor::Finding::in_category(
            crate::doctor::Category::Services,
            check,
            "A service run on a schedule",
            crate::doctor::Verdict::Warn(problem.clone().with_detail("one\ntwo")),
        )
    };
    let report = crate::model::DoctorReport {
        overall: crate::doctor::Overall::Degraded,
        findings: vec![finding(&checked("configarr")), finding("services.other")],
        rehearsed: false,
    };
    let crate::app::Outcome::Doctor(keyed) = crate::app::Outcome::Doctor(report).for_a_key() else {
        unreachable!("a doctor report stays one");
    };
    let detail = |finding: &crate::doctor::Finding| match &finding.verdict {
        crate::doctor::Verdict::Warn(problem) => problem.detail.clone(),
        _ => None,
    };
    let [scheduled, other] = keyed.findings.as_slice() else {
        unreachable!("both findings kept: {keyed:?}");
    };
    assert_eq!(detail(scheduled), None);
    assert!(
        matches!(&scheduled.verdict, crate::doctor::Verdict::Warn(kept)
            if kept.code == problem.code && kept.summary == problem.summary
                && kept.meaning == problem.meaning && kept.remedies == problem.remedies),
        "{scheduled:?}"
    );
    assert_eq!(detail(other).as_deref(), Some("one\ntwo"));
}

#[test]
fn an_update_drops_each_service_it_no_longer_runs_on_a_schedule() {
    let was = installed(&[
        ("configarr", Some("@daily")),
        ("recyclarr", Some("@hourly")),
    ]);
    let would = installed(&[("configarr", Some("@weekly")), ("recyclarr", None)]);
    let (Some(was), Some(would)) = (was.first(), would.first()) else {
        unreachable!("one plugin each");
    };
    assert_eq!(super::dropped(was, would), ["recyclarr"]);
    assert!(super::dropped(would, was).is_empty());
}

#[test]
fn a_key_reads_a_failing_runs_finding_without_its_lines_and_a_passing_one_as_it_was() {
    let problem = super::failed("configarr", &ran(Some(1), &["one"]))
        .unwrap_or_else(|| unreachable!("failed"));
    let finding = |service: &str, verdict| {
        crate::doctor::Finding::in_category(
            crate::doctor::Category::Services,
            &checked(service),
            "A service run on a schedule",
            verdict,
        )
    };
    let passing = finding("recyclarr", crate::doctor::Verdict::Pass { note: None });
    let report = crate::model::DoctorReport {
        overall: crate::doctor::Overall::Broken,
        findings: vec![
            finding(
                "configarr",
                crate::doctor::Verdict::Fail(problem.with_detail("one")),
            ),
            passing.clone(),
        ],
        rehearsed: false,
    };
    let crate::app::Outcome::Doctor(keyed) = crate::app::Outcome::Doctor(report).for_a_key() else {
        unreachable!("a doctor report stays one");
    };
    let [failing, kept] = keyed.findings.as_slice() else {
        unreachable!("both findings kept: {keyed:?}");
    };
    assert!(
        matches!(&failing.verdict, crate::doctor::Verdict::Fail(problem) if problem.detail.is_none()),
        "{failing:?}"
    );
    assert_eq!(*kept, passing);
}

#[test]
fn a_key_reads_each_service_without_the_lines_its_last_run_wrote() {
    let service = crate::model::ScheduledService {
        id: "configarr".to_owned(),
        name: "Configarr".to_owned(),
        running: false,
        fired: true,
        health: crate::model::RunHealth::Failed,
        last: Some(ran(Some(1), &["a line it wrote"])),
        skipped: 0,
    };

    let read = super::unlogged(vec![service.clone()]);

    assert_eq!(
        read,
        vec![crate::model::ScheduledService {
            last: Some(ran(Some(1), &[])),
            ..service
        }]
    );
}

#[test]
fn a_key_reads_a_run_and_a_status_without_the_lines_and_anything_else_as_it_was() {
    let report = crate::model::RunReport {
        service: "configarr".to_owned(),
        rehearsed: false,
        due: true,
        ran: Some(ran(Some(0), &["a line it wrote"])),
        skipped: 0,
        held: None,
    };
    let status = crate::model::StatusReport {
        forms: Vec::new(),
        active_forms: Vec::new(),
        filtered: Vec::new(),
        waiting: Vec::new(),
        condition: crate::docker::Condition::Active,
        services: Vec::new(),
        undeclared: Vec::new(),
        scheduled: vec![crate::model::ScheduledService {
            id: "configarr".to_owned(),
            name: "Configarr".to_owned(),
            running: false,
            fired: true,
            health: crate::model::RunHealth::Succeeded,
            last: Some(ran(Some(0), &["a line it wrote"])),
            skipped: 0,
        }],
        disturbs: crate::model::Disturbances::all(std::time::Duration::ZERO),
    };
    let other =
        crate::app::Outcome::Preview(crate::stack::closure::Plan::one("configarr", "notify"));

    let crate::app::Outcome::Ran(ran_keyed) = crate::app::Outcome::Ran(report).for_a_key() else {
        unreachable!("a run stays one");
    };
    let crate::app::Outcome::Status(status_keyed) = crate::app::Outcome::Status(status).for_a_key()
    else {
        unreachable!("a status stays one");
    };

    assert_eq!(ran_keyed.ran.map(|one| one.log), Some(Vec::new()));
    assert_eq!(
        status_keyed
            .scheduled
            .into_iter()
            .map(|one| one.last.map(|last| last.log))
            .collect::<Vec<_>>(),
        vec![Some(Vec::new())]
    );
    assert_eq!(other.clone().for_a_key(), other);
}
