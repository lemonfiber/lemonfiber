//! The doctor reads what the request gate refused and removed, on a stack that runs the
//! gate, and reports each entry once.

use std::path::Path;
use std::sync::Arc;

use lemonfiber_core::app::diagnose;
use lemonfiber_core::config::Settings;
use lemonfiber_core::doctor::{Narrowing, Verdict};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_ports::docker::{Health, Lifecycle};
use lemonfiber_sidecar::gate::{Kept, Outcome, Record};

#[tokio::test]
async fn a_refusal_is_reported_once_through_the_whole_diagnosis() {
    let stack: &'static Path =
        Box::leak(crate::common::stack::with_the_gate("record-read").into_boxed_path());
    let gate = stack.join("config/request-gate");
    let _ = std::fs::create_dir_all(&gate);
    let refused = Record::default().with(
        60,
        "sonarr",
        "POST",
        "/sonarr/api/v3/command",
        Outcome::Refused,
        Kept::standard(),
    );
    let _ = std::fs::write(gate.join("record.json"), refused.written());
    let config = lemonfiber_fixtures::scratch::Scratch::named("gated-record-config").kept();
    let _ = std::fs::create_dir_all(&config);
    let env = config.join(".env");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    let ctx = lemonfiber_testing::a_context()
        .over(Source::External(stack))
        .runner(Arc::new(lemonfiber_fixtures::ports::Idle))
        .engine(Arc::new(Reporting::holding(
            &[],
            Lifecycle::Exited,
            Health::None,
        )))
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build();
    let narrowing = Narrowing::Check("services.request-gate-record".to_owned());

    let first = diagnose(&ctx, &narrowing, false).await;
    let second = diagnose(&ctx, &narrowing, false).await;
    let read = std::fs::read_to_string(config.join("gate-read.json")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(stack);
    let _ = std::fs::remove_dir_all(&config);

    let verdicts = |report: Result<lemonfiber_core::model::DoctorReport, _>| -> Vec<Verdict> {
        report
            .map(|report| report.findings.into_iter().map(|one| one.verdict).collect())
            .unwrap_or_default()
    };
    assert!(
        matches!(verdicts(first).as_slice(), [Verdict::Warn(_)]),
        "a refusal was not raised"
    );
    assert!(
        matches!(verdicts(second).as_slice(), [Verdict::Pass { .. }]),
        "a refusal already read was raised again"
    );
    assert_eq!(read.trim(), "1");
}
