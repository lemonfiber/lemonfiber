//! A plugin whose part has no port, proved by running its image's own program.

use lemonfiber_fixtures::proving::Proving;

use super::*;

const RUNNING: &str = r#"
schema_version = 1

[plugin]
id          = "quality"
name        = "Quality sync"
version     = "1.0.0"
description = "Keeps quality profiles in step with the guides"
without_it  = "Profiles drift from the guides"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "qualitysync"
name        = "Quality sync"
image       = "example.invalid/qualitysync"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "8.7.3"
criticality = "optional"
config_path = "/config"
user_mode   = "uid"

[[proof]]
id      = "qualitysync.runs"
title   = "It starts as the service runs it"
run     = { args = ["--version"] }
expect  = { exit = 0, stdout_starts_with = "v8." }
why     = "It has no port, so the live question is whether its own program starts."
"#;

/// The plugin's source, with one starting file for its service.
fn starting(name: &str) -> PathBuf {
    let at = source(name, RUNNING);
    let _ = std::fs::create_dir_all(at.join("config/qualitysync"));
    let _ = std::fs::write(at.join("config/qualitysync/recyclarr.yml"), "sonarr: {}\n");
    at
}

/// A context that proves, whose engine runs the image's own program with `program`.
fn running(name: &str, program: &Arc<Proving>) -> Ctx {
    proving(
        name,
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    )
    .with_program(Arc::clone(program) as Arc<dyn crate::ports::program::Program>)
}

/// Where the starting files kept for the plugin are.
fn kept_file(ctx: &Ctx) -> PathBuf {
    ctx.settings
        .env_file
        .clone()
        .unwrap_or_default()
        .with_file_name("plugin-starting")
        .join("quality/config/qualitysync/recyclarr.yml")
}

#[tokio::test]
async fn a_part_proved_by_its_own_program_is_installed_once_the_run_holds() {
    let program = Proving::exiting(0, "v8.7.3 (Branch.master)\n");
    let ctx = running("run-proved", &program);

    let outcome = installing(&ctx, &starting("run-proved")).await;

    assert_eq!(
        proving::verdicts(outcome)
            .into_iter()
            .map(|verdict| came_to(verdict.as_ref()))
            .collect::<Vec<_>>(),
        vec!["passed".to_owned()]
    );
    let asked = program.asked();
    assert_eq!(asked.len(), 1);
    assert!(asked
        .first()
        .is_some_and(|one| one.user.as_deref() == Some("1000:1000")
            && one.args == vec!["--version".to_owned()]
            && one.configuration.is_some()));
    let record = read(&record_of(&ctx));
    assert!(
        record.contains("\"run_proofs\"") && record.contains("\"starting\":\"sha256:"),
        "{record}"
    );
    assert_eq!(read(&kept_file(&ctx)), "sonarr: {}\n");
}

#[tokio::test]
async fn a_run_that_does_not_hold_leaves_nothing_installed() {
    let program = Proving::exiting(1, "error");
    let ctx = running("run-refuted", &program);

    let outcome = installing(&ctx, &starting("run-refuted")).await;

    let install = report(outcome).and_then(|one| one.install);
    assert!(install.as_ref().is_some_and(|install| !install.recorded));
    assert!(install.is_some_and(|install| install
        .proofs
        .iter()
        .any(|one| matches!(&one.came_to, Some(Verdict::Failed { faults }) if faults.iter().any(|fault| fault.contains("exited 1"))))));
    assert!(!kept_file(&ctx).exists());
}

#[tokio::test]
async fn proving_again_runs_it_with_the_starting_files_kept_at_install() {
    let ctx = running("run-reproved", &Proving::exiting(0, "v8.7.3"));
    assert!(installing(&ctx, &starting("run-reproved"))
        .await
        .is_ok_and(|done| done.install.is_some_and(|install| install.recorded)));

    let again = Proving::exiting(0, "v8.7.3");
    let ctx = ctx.with_program(Arc::clone(&again) as Arc<dyn crate::ports::program::Program>);
    let asked = Asked::Prove {
        plugin: "quality".to_owned(),
        consent: super::super::Consent::default(),
    };
    let proved = report(answered(&ctx, asked.clone()).await).and_then(|one| one.proof);
    assert!(
        proved.as_ref().is_some_and(|proof| proof
            .proofs
            .iter()
            .any(|one| one.proof == "qualitysync.runs"
                && one.asks == "runs --version"
                && one.came_to == Some(Verdict::Passed))),
        "{proved:?}"
    );
    assert_eq!(again.asked().len(), 1);

    let _ = std::fs::write(kept_file(&ctx), "edited");
    let proved = report(answered(&ctx, asked).await).and_then(|one| one.proof);
    assert!(
        proved.is_some_and(|proof| proof.proofs.iter().any(|one| matches!(&one.came_to,
            Some(Verdict::Unproven { why }) if why.contains("not as they were")))),
    );

    assert!(removing(&ctx, "quality").await.is_ok());
    assert!(!kept_file(&ctx).exists());
}

#[tokio::test]
async fn the_doctor_runs_it_once_and_keeps_what_it_came_to() {
    let ctx = running("run-doctored", &Proving::exiting(0, "v8.7.3"));
    assert!(installing(&ctx, &starting("run-doctored"))
        .await
        .is_ok_and(|done| done.install.is_some_and(|install| install.recorded)));
    let services = crate::doctor::Narrowing::Category(crate::doctor::Category::Services);

    let first = Proving::exiting(0, "v8.7.3");
    let ctx = ctx.with_program(Arc::clone(&first) as Arc<dyn crate::ports::program::Program>);
    let found = crate::app::engine::diagnose(&ctx, &services, false).await;
    assert!(
        found.as_ref().is_ok_and(|report| report
            .findings
            .iter()
            .any(|one| one.check == "quality:qualitysync.runs"
                && matches!(one.verdict, crate::doctor::Verdict::Pass { .. }))),
        "{found:?}"
    );
    assert_eq!(first.asked().len(), 1);

    let later = Proving::exiting(1, "broken");
    let ctx = ctx.with_program(Arc::clone(&later) as Arc<dyn crate::ports::program::Program>);
    let _ = crate::app::engine::diagnose(&ctx, &services, false).await;
    assert!(later.asked().is_empty(), "a kept verdict is not run again");
}

#[tokio::test]
async fn an_update_runs_the_new_version_and_keeps_its_starting_files() {
    let ctx = running("run-updated", &Proving::exiting(0, "v8.7.3"));
    assert!(installing(&ctx, &starting("run-updated"))
        .await
        .is_ok_and(|done| done.install.is_some_and(|install| install.recorded)));

    let newer = source(
        "run-updated-next",
        &RUNNING.replace(r#"version     = "1.0.0""#, r#"version     = "1.0.1""#),
    );
    let _ = std::fs::create_dir_all(newer.join("config/qualitysync"));
    let _ = std::fs::write(
        newer.join("config/qualitysync/recyclarr.yml"),
        "radarr: {}\n",
    );
    let again = Proving::exiting(0, "v8.7.4");
    let ctx = ctx.with_program(Arc::clone(&again) as Arc<dyn crate::ports::program::Program>);

    let account = update(updating(&ctx, &newer).await);

    assert!(account.is_some_and(|account| account.install.recorded));
    assert_eq!(again.asked().len(), 1);
    assert_eq!(on(&ctx).await.as_deref(), Some("1.0.1"));
    assert_eq!(read(&kept_file(&ctx)), "radarr: {}\n");
}

#[tokio::test]
async fn a_run_proof_whose_service_is_not_placed_asks_nothing() {
    let ctx = running("run-unplaced", &Proving::exiting(0, "v8.7.3"));
    let Ok(manifest) = lemonfiber_plugin::Manifest::from_toml(RUNNING) else {
        return;
    };
    let mut installed = crate::plugin::Installed::of(&manifest);
    installed.services.clear();
    let mut stated = crate::plugin::proofs(&manifest);
    let at = starting("run-unplaced");
    super::super::proving::asked(&ctx, &manifest, &installed, &at, &at, &mut stated).await;
    assert!(
        stated.iter().any(|one| matches!(&one.came_to,
            Some(Verdict::Unproven { why }) if why.contains("does not settle one service"))),
        "{stated:?}"
    );
}

#[tokio::test]
async fn a_run_proof_proved_again_with_no_service_or_nowhere_kept_is_unproven() {
    let ctx = running("run-rerun-edges", &Proving::exiting(0, "v8.7.3"));
    let Ok(manifest) = lemonfiber_plugin::Manifest::from_toml(RUNNING) else {
        return;
    };
    let installed = crate::plugin::Installed::of(&manifest);
    let Some(run) = installed.run_proofs.first().cloned() else {
        return;
    };
    let mut unplaced = installed.clone();
    unplaced.services.clear();
    assert!(matches!(
        super::super::reproving::rerun(&ctx, &unplaced, &run).await,
        Verdict::Unproven { why } if why.contains("not in the record")
    ));
    let mut nowhere = ctx.clone();
    nowhere.settings.env_file = None;
    assert!(matches!(
        super::super::reproving::rerun(&nowhere, &installed, &run).await,
        Verdict::Unproven { why } if why.contains("nowhere configured")
    ));
}
