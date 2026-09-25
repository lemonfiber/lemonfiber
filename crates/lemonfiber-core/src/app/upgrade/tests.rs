use std::sync::Arc;

use super::upgrade;
use super::Ctx;
use crate::model::Triggered;
use crate::quality::Preset;
use crate::test_support::{a_context, nowhere, SeedFs};
use lemonfiber_fixtures::http::Fake;

/// The Servarr config file `SeedFs` hands back for any \*arr, carrying a readable
/// key so a target opens.
const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

/// A context over the real stack (which names Sonarr and Radarr), the given
/// filesystem, and HTTP that answers the upgrade POSTs from `replies`.
fn ctx(fs: Arc<SeedFs>, replies: Vec<(u16, &'static str)>) -> Ctx {
    a_context()
        .build()
        .with_filesystem(fs)
        .with_http(Fake::scripted(replies))
}

fn outcomes(report: &crate::model::UpgradeReport) -> Vec<Option<&Triggered>> {
    report
        .media
        .iter()
        .map(|media| media.outcome.as_ref())
        .collect()
}

#[tokio::test]
async fn unconfirmed_states_the_cost_per_media_and_triggers_nothing() {
    let report = upgrade(&ctx(Arc::new(SeedFs::keyed(None, None)), vec![]), false)
        .await
        .unwrap_or_default();
    assert!(!report.confirmed);
    // Both resolution media types, each with the default preset's cost stated and
    // no outcome — nothing was touched.
    assert_eq!(
        report.media.len(),
        2,
        "television and film are both covered"
    );
    assert!(report.media.iter().all(|media| {
        media.preset == Preset::default_preset().label()
            && media.size_per_hour.contains("GB")
            && media.outcome.is_none()
    }));
}

#[tokio::test]
async fn a_confirmed_upgrade_starts_a_research_on_each_resolution_arr() {
    // Both media *arrs accept the command; music and index services are left out.
    let report = upgrade(
        &ctx(
            Arc::new(SeedFs::keyed(Some(KEYED), None)),
            vec![(201, ""), (201, "")],
        ),
        true,
    )
    .await
    .unwrap_or_default();
    assert!(report.confirmed);
    assert_eq!(report.media.len(), 2, "only Sonarr and Radarr are asked");
    assert!(outcomes(&report)
        .iter()
        .all(|outcome| matches!(outcome, Some(Triggered::Started))));
}

#[tokio::test]
async fn a_per_type_choice_states_each_types_own_preset() {
    // The report speaks per media type: a maximum-for-film, space-saving-for-tv
    // split is not flattened to one figure.
    let env = lemonfiber_fixtures::scratch::Scratch::named("upgrade-split");
    let _ = std::fs::remove_dir_all(&env);
    let env = env.join(".env");
    let mut selection = crate::quality::Selection::everywhere(Preset::Balanced);
    selection.set_type("movies", Preset::Maximum);
    selection.set_type("tv", Preset::SpaceSaving);
    let _ = crate::config::store::write(
        &env.with_file_name("quality.json"),
        &serde_json::to_string(&selection).unwrap_or_default(),
    );
    let mut context = ctx(Arc::new(SeedFs::keyed(None, None)), vec![]);
    context.settings.env_file = Some(env);

    let report = upgrade(&context, false).await.unwrap_or_default();
    let presets: Vec<&str> = report
        .media
        .iter()
        .map(|media| media.preset.as_str())
        .collect();
    assert!(presets.contains(&Preset::Maximum.label()));
    assert!(presets.contains(&Preset::SpaceSaving.label()));
}

#[tokio::test]
async fn an_arr_not_yet_started_is_reported_not_started() {
    // No key on disk: the service has not finished starting, so it is not a fault.
    let report = upgrade(&ctx(Arc::new(SeedFs::keyed(None, None)), vec![]), true)
        .await
        .unwrap_or_default();
    assert_eq!(report.media.len(), 2);
    assert!(outcomes(&report)
        .iter()
        .all(|outcome| matches!(outcome, Some(Triggered::NotStarted))));
}

#[tokio::test]
async fn an_arr_that_refuses_the_command_is_reported_failed() {
    let report = upgrade(
        &ctx(
            Arc::new(SeedFs::keyed(Some(KEYED), None)),
            vec![(500, "boom"), (500, "boom")],
        ),
        true,
    )
    .await
    .unwrap_or_default();
    assert!(outcomes(&report)
        .iter()
        .all(|outcome| matches!(outcome, Some(Triggered::Failed { .. }))));
}

#[tokio::test]
async fn a_confirmed_upgrade_over_an_unreadable_stack_is_an_error() {
    let bad = a_context().over(nowhere()).build();
    assert!(upgrade(&bad, true).await.is_err());
}
