//! Whole seed passes: what one carries, what it keeps for the next, and what refuses one.

use super::*;

#[test]
fn a_non_seed_outcome_carries_no_seed_report() {
    let version = Outcome::Version(VersionReport {
        binary: env!("CARGO_PKG_VERSION").to_owned(),
        supported_schema: vec![1],
        stack: "0.1.0".to_owned(),
        compose: None,
        attestation_status: None,
        changelog: crate::changelog::Notes::unread(),
    });
    assert!(seeded(Ok(version)).is_none());
}

#[tokio::test]
async fn the_baseline_persists_across_runs() {
    // The baseline is the one seed artifact kept between runs. A value an earlier
    // run recorded is pre-seeded here; two more passes — neither reaching a
    // service, so neither changing it — must load it, leave it, and save it back
    // unchanged, the round trip the drift policy is built to read.
    let env = config_scratch("baseline-across-runs");
    let baseline = env.with_file_name("baseline.json");
    let recorded =
        r#"{"services":{"sonarr":{"downloadclient:sabnzbd:8080":{"value":"tv","at":"1"}}}}"#;
    let _ = crate::config::store::write(&baseline, recorded);

    let first = seed_ctx(false, None, Some(env.to_path_buf()));
    let _ = dispatch(Command::Seed, &first).await;
    let second = seed_ctx(false, None, Some(env.to_path_buf()));
    let _ = dispatch(Command::Seed, &second).await;

    let read_back = std::fs::read_to_string(&baseline).unwrap_or_default();
    assert!(
        read_back.contains(r#""value":"tv""#) && read_back.contains(r#""at":"1""#),
        "the recorded value and its timestamp survive both passes unchanged: {read_back}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

#[tokio::test]
async fn seed_reports_an_unreadable_stack_rather_than_guessing() {
    let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
    let ctx = a_context()
        .engine(Arc::new(Reporting::default()))
        .over(nowhere)
        .build();
    let outcome = dispatch(Command::Seed, &ctx).await;
    assert_eq!(
        outcome.err().map(|problem| problem.code),
        Some(crate::error::codes::stack::STACK_UNREADABLE)
    );
}
