use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::scratch::Scratch;

use super::{OverlayCheck, Verdict};
use crate::doctor::{Check, Finding};

fn machine(name: &str) -> (Scratch, PathBuf) {
    let dir = Scratch::new(&format!("overlay-check-{name}"));
    let _ = std::fs::create_dir_all(dir.as_ref());
    let settings = dir.join(".env");
    (dir, settings)
}

async fn found(settings: Option<PathBuf>) -> Vec<Finding> {
    OverlayCheck::new(Arc::new(lemonfiber_adapters::Disk), settings)
        .run()
        .await
}

fn problem<'a>(found: &'a [Finding], check: &str) -> &'a crate::error::Problem {
    match found
        .iter()
        .find(|one| one.check == check)
        .map(|one| &one.verdict)
    {
        Some(Verdict::Warn(problem)) => problem,
        other => unreachable!("{check} is a warning, not {other:?}"),
    }
}

#[tokio::test]
async fn no_settings_skips_and_no_overlay_passes_saying_none_is_merged() {
    let skipped = found(None).await;
    assert!(matches!(
        skipped.first().map(|one| &one.verdict),
        Some(Verdict::Skipped { .. })
    ));

    let (_dir, settings) = machine("none");
    let passed = found(Some(settings)).await;
    let checks: Vec<&str> = passed.iter().map(|one| one.check.as_str()).collect();
    assert_eq!(checks, ["config.overlay"]);
    assert!(matches!(
        passed.first().map(|one| &one.verdict),
        Some(Verdict::Pass { note: Some(note) }) if note.contains("no overlay")
    ));
}

#[tokio::test]
async fn an_overlay_is_named_unvetted_with_every_service_it_changes() {
    let (_dir, settings) = machine("unvetted");
    let overlay = settings.with_file_name("overlay.yml");
    assert!(std::fs::write(&overlay, "services:\n  sonarr: {}\n  radarr: {}\n").is_ok());

    let named = found(Some(settings.clone())).await;
    let said = problem(&named, "config.overlay");
    assert_eq!(said.code.as_str(), "CONFIG-8");
    assert!(
        said.summary.contains(&overlay.display().to_string()),
        "{}",
        said.summary
    );
    assert!(
        said.meaning.contains("changes sonarr, radarr"),
        "{}",
        said.meaning
    );

    assert!(std::fs::write(&overlay, "volumes: {}\n").is_ok());
    let unnamed = found(Some(settings)).await;
    let said = problem(&unnamed, "config.overlay");
    assert!(
        said.meaning.contains("changes no service by name"),
        "{}",
        said.meaning
    );
}

#[tokio::test]
async fn a_path_under_the_retired_setting_is_named_with_where_to_move_it() {
    let (_dir, settings) = machine("retired");
    assert!(std::fs::write(&settings, "LEMONFIBER_OVERLAY=  \n").is_ok());
    let blank = found(Some(settings.clone())).await;
    assert!(blank
        .iter()
        .all(|one| one.check != "config.overlay-retired"));

    assert!(std::fs::write(&settings, "LEMONFIBER_OVERLAY=/srv/old.yml\n").is_ok());
    let named = found(Some(settings.clone())).await;
    let said = problem(&named, "config.overlay-retired");
    assert_eq!(said.code.as_str(), "CONFIG-9");
    assert!(said.summary.contains("/srv/old.yml"), "{}", said.summary);
    let at = settings.with_file_name("overlay.yml").display().to_string();
    let moved = said.remedies.iter().any(|remedy| {
        remedy
            .action
            .contains(&format!("Move /srv/old.yml to {at}"))
    });
    assert!(moved, "{:?}", said.remedies);
}
