use lemonfiber_contract::capabilities::library::curate;
use lemonfiber_contract::wire::Kind as Refused;

use super::upgrade;
use super::Ctx;
use crate::model::{Total, Triggered};
use crate::ports::media::Format;
use crate::ports::service::CutoffUnmet;
use crate::quality::Preset;
use crate::test_support::adapted;
use crate::test_support::adapted::refused;
use crate::test_support::curators::{answered, refusing, taken, ALL};
use crate::test_support::{a_context, nowhere};
use lemonfiber_fixtures::http::{Answer, Fake};

/// What every curator holds below its cutoff in these cases: two hours of it.
const BELOW: CutoffUnmet = CutoffUnmet {
    items: 2,
    seconds: 7_200,
};

/// A context over the real stack, which names a television, a film and a music curator,
/// each moved into its plugin and asked through its adapter where `reached`, with HTTP
/// answering every curator's upgrade search with `searched` and what it holds below its
/// cutoff with `below`.
fn ctx(reached: bool, searched: Answer, below: Answer) -> Ctx {
    ctx_of(&ALL, reached, searched, below)
}

/// A context as [`ctx`] builds it, naming `curators` rather than the curators of
/// television, film and music.
fn ctx_of(curators: &[adapted::Adapted], reached: bool, searched: Answer, below: Answer) -> Ctx {
    static BUILT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nth = BUILT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("upgrade-{nth}")).kept();
    let _ = std::fs::create_dir_all(&dir);
    let mut context = a_context().build().with_http(Fake::by_path(vec![
        (
            adapted::path_of(curate::CAPABILITY, curate::MAJOR, "search_upgrades"),
            searched,
        ),
        (
            adapted::path_of(curate::CAPABILITY, curate::MAJOR, "cutoff_unmet"),
            below,
        ),
    ]));
    context.settings.env_file = Some(dir.join(".env"));
    if reached {
        return adapted::reaching(context, curators);
    }
    for curator in curators {
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

fn totals(report: &crate::model::UpgradeReport) -> Vec<(&str, &Total)> {
    report
        .media
        .iter()
        .map(|media| (media.media_type.as_str(), &media.total))
        .collect()
}

#[tokio::test]
async fn unconfirmed_states_each_kinds_bar_and_triggers_nothing() {
    let report = upgrade(&ctx(false, taken(), answered(&BELOW)), false)
        .await
        .unwrap_or_default();
    assert!(!report.confirmed);
    let bars: Vec<(&str, &str)> = report
        .media
        .iter()
        .map(|media| (media.media_type.as_str(), media.preset.as_str()))
        .collect();
    assert_eq!(bars.len(), 3, "television, film and music are all covered");
    for (media_type, preset) in bars {
        let expected = if media_type == "music" {
            Format::default_format().label()
        } else {
            Preset::default_preset().label()
        };
        assert_eq!(preset, expected, "{media_type}");
    }
    assert!(outcomes(&report).iter().all(Option::is_none));
}

#[tokio::test]
async fn a_curator_of_books_is_left_out_having_no_quality_to_upgrade_to() {
    let [music, film, television] = ALL;
    let context = ctx_of(
        &[
            music,
            film,
            television,
            crate::test_support::collecting::BOOKS,
        ],
        true,
        taken(),
        answered(&BELOW),
    );
    let manifest = context.stack.checked_manifest(context.today()).ok();
    let mut kinds = Vec::new();
    if let Some(manifest) = manifest {
        for target in
            super::curator_targets(&context, &super::fillers_here(&context, &manifest)).await
        {
            kinds.extend(target.kind);
        }
    }

    let report = upgrade(&context, false).await.unwrap_or_default();

    assert!(
        kinds.contains(&crate::ports::media::TitleKind::Books),
        "{kinds:?}"
    );
    let covered: Vec<&str> = report
        .media
        .iter()
        .map(|media| media.media_type.as_str())
        .collect();
    assert_eq!(covered.len(), 3, "{covered:?}");
    assert!(!covered.contains(&crate::ports::media::TitleKind::Books.media_type()));
}

#[tokio::test]
async fn each_kinds_total_is_the_runtime_below_its_cutoff_at_the_bar_in_force() {
    let report = upgrade(&ctx(true, taken(), answered(&BELOW)), false)
        .await
        .unwrap_or_default();
    for (media_type, total) in totals(&report) {
        let rate = if media_type == "music" {
            Format::default_format().bytes_per_hour()
        } else {
            Preset::default_preset().bytes_per_hour()
        };
        assert_eq!(total, &Total::Bytes(2 * rate), "{media_type}");
    }
}

#[tokio::test]
async fn a_curator_that_does_not_say_what_it_holds_below_its_cutoff_leaves_the_total_unknown() {
    let report = upgrade(&ctx(true, taken(), refused(Refused::Unsupported)), false)
        .await
        .unwrap_or_default();
    assert!(totals(&report).iter().all(|(_, total)| matches!(
        total,
        Total::Unknown(why) if why.contains("does not say what it holds below its cutoff")
    )));
}

#[tokio::test]
async fn a_curator_that_refuses_or_has_not_started_leaves_the_total_unknown_saying_why() {
    let refusing_report = upgrade(&ctx(true, taken(), refusing()), false)
        .await
        .unwrap_or_default();
    assert!(totals(&refusing_report).iter().all(|(_, total)| matches!(
        total,
        Total::Unknown(why) if why.contains("could not be read:")
    )));
    let unstarted = upgrade(&ctx(false, taken(), answered(&BELOW)), false)
        .await
        .unwrap_or_default();
    assert!(totals(&unstarted).iter().all(|(_, total)| matches!(
        total,
        Total::Unknown(why) if why.contains("has not finished starting")
    )));
}

#[tokio::test]
async fn a_confirmed_upgrade_starts_a_research_on_each_curator_music_among_them() {
    let report = upgrade(&ctx(true, taken(), answered(&BELOW)), true)
        .await
        .unwrap_or_default();
    assert!(report.confirmed);
    assert_eq!(
        report.media.len(),
        3,
        "television, film and music are asked"
    );
    assert!(outcomes(&report)
        .iter()
        .all(|outcome| matches!(outcome, Some(Triggered::Started))));
}

#[tokio::test]
async fn a_per_type_choice_states_each_types_own_preset() {
    // The report speaks per media type: a maximum-for-film, space-saving-for-tv
    // split is not flattened to one figure.
    let context = ctx(false, taken(), answered(&BELOW));
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
    let report = upgrade(&ctx(false, taken(), answered(&BELOW)), true)
        .await
        .unwrap_or_default();
    assert_eq!(report.media.len(), 3);
    assert!(outcomes(&report)
        .iter()
        .all(|outcome| matches!(outcome, Some(Triggered::NotStarted))));
}

#[tokio::test]
async fn a_curator_that_refuses_the_command_is_reported_failed() {
    let report = upgrade(&ctx(true, refusing(), answered(&BELOW)), true)
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
