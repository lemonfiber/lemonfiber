use lemonfiber_contract::capabilities::library::curate;

use super::upgrade;
use super::Ctx;
use crate::model::Triggered;
use crate::quality::Preset;
use crate::test_support::adapted;
use crate::test_support::curators::{refusing, taken, ALL};
use crate::test_support::{a_context, nowhere};
use lemonfiber_fixtures::http::{Answer, Fake};

/// A context over the real stack, which names a television, a film and a music curator,
/// each moved into its plugin and asked through its adapter where `reached`, with HTTP
/// answering every curator's upgrade search with `searched`.
fn ctx(reached: bool, searched: Answer) -> Ctx {
    static BUILT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nth = BUILT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("upgrade-{nth}")).kept();
    let _ = std::fs::create_dir_all(&dir);
    let mut context = a_context().build().with_http(Fake::by_path(vec![(
        adapted::path_of(curate::CAPABILITY, curate::MAJOR, "search_upgrades"),
        searched,
    )]));
    context.settings.env_file = Some(dir.join(".env"));
    if reached {
        return adapted::reaching(context, &ALL);
    }
    for curator in ALL {
        curator.recorded(&context);
    }
    context
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
    let report = upgrade(&ctx(false, taken()), false)
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
async fn a_confirmed_upgrade_starts_a_research_on_each_resolution_curator() {
    // Both media curators accept the command; music and index services are left out.
    let report = upgrade(&ctx(true, taken()), true).await.unwrap_or_default();
    assert!(report.confirmed);
    assert_eq!(
        report.media.len(),
        2,
        "only the TV and movie curators are asked"
    );
    assert!(outcomes(&report)
        .iter()
        .all(|outcome| matches!(outcome, Some(Triggered::Started))));
}

#[tokio::test]
async fn a_per_type_choice_states_each_types_own_preset() {
    // The report speaks per media type: a maximum-for-film, space-saving-for-tv
    // split is not flattened to one figure.
    let context = ctx(false, taken());
    let mut selection = crate::quality::Selection::everywhere(Preset::Balanced);
    selection.set_type("movies", Preset::Maximum);
    selection.set_type("tv", Preset::SpaceSaving);
    let _ = crate::config::store::write(
        &context
            .settings
            .env_file
            .clone()
            .unwrap_or_default()
            .with_file_name("quality.json"),
        &serde_json::to_string(&selection).unwrap_or_default(),
    );

    let report = upgrade(&context, false).await.unwrap_or_default();
    let presets: Vec<&str> = report
        .media
        .iter()
        .map(|media| media.preset.as_str())
        .collect();
    assert!(presets.contains(&Preset::Maximum.label()));
    assert!(presets.contains(&Preset::SpaceSaving.label()));
}

/// No adapter runs: the service has not finished starting, so it is not a fault.
#[tokio::test]
async fn a_curator_not_yet_started_is_reported_not_started() {
    let report = upgrade(&ctx(false, taken()), true)
        .await
        .unwrap_or_default();
    assert_eq!(report.media.len(), 2);
    assert!(outcomes(&report)
        .iter()
        .all(|outcome| matches!(outcome, Some(Triggered::NotStarted))));
}

#[tokio::test]
async fn a_curator_that_refuses_the_command_is_reported_failed() {
    let report = upgrade(&ctx(true, refusing()), true)
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
