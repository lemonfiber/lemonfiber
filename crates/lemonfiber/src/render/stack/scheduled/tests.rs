use lemonfiber_core::docker::Condition;
use lemonfiber_core::model::StatusReport;

use super::super::status;

#[test]
fn each_run_and_exit_service_says_how_its_last_run_ended_and_a_failed_one_its_last_lines() {
    use lemonfiber_core::instant::Instant;
    use lemonfiber_core::model::ScheduledService;
    use lemonfiber_core::scheduled::kept::ScheduledRun;

    let ran = |exit| ScheduledRun {
        started: Instant::from_seconds(1_759_400_000),
        ended: Instant::from_seconds(1_759_400_060),
        exit,
        log: vec!["refused: 401".to_owned()],
    };
    let one =
        |id: &str, last: Option<ScheduledRun>, running: bool, skipped: u32| ScheduledService {
            id: id.to_owned(),
            name: id.to_owned(),
            running,
            fired: true,
            health: lemonfiber_core::model::RunHealth::of(last.as_ref()),
            last,
            skipped,
        };
    let report = StatusReport {
        forms: Vec::new(),
        active_forms: Vec::new(),
        filtered: Vec::new(),
        condition: Condition::Degraded,
        undeclared: Vec::new(),
        services: Vec::new(),
        scheduled: vec![
            one("configarr", Some(ran(Some(2))), false, 2),
            one("fine", Some(ran(Some(0))), false, 0),
            one("ended", Some(ran(None)), false, 1),
            one("new", None, false, 0),
            one("busy", None, true, 0),
            ScheduledService {
                fired: false,
                ..one("lost", None, false, 0)
            },
        ],
        disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
        waiting: Vec::new(),
    };
    let text = status(&report).text();
    assert!(text.contains("run on a schedule:"), "{text}");
    assert!(
        text.contains("configarr      failed (2) 2025-10-02"),
        "{text}"
    );
    assert!(text.contains("    refused: 401"), "{text}");
    assert!(
        text.contains("2 times skipped while a run was going"),
        "{text}"
    );
    assert!(text.contains("1 time skipped"), "{text}");
    assert!(text.contains("fine           ran 2025-10-02"), "{text}");
    assert!(text.contains("ended          failed 2025-10-02"), "{text}");
    assert!(text.contains("new            not run yet"), "{text}");
    assert!(text.contains("busy           running"), "{text}");
    assert!(
        text.contains("lost           not run yet, and nothing on this machine runs it"),
        "{text}"
    );
    assert!(!text.contains("busy           running, and"), "{text}");
    assert_eq!(text.matches("refused: 401").count(), 2, "{text}");

    let none = StatusReport {
        scheduled: Vec::new(),
        ..report
    };
    assert!(!status(&none).text().contains("run on a schedule"));
}
