use std::sync::Arc;

use lemonfiber_contract::capabilities::media::serve;
use lemonfiber_contract::Contracted;
use lemonfiber_fixtures::http::{Answer, Fake};

use super::{SeasonsCheck, CHECK};
use crate::doctor::{Category, Check, Finding, Mend, Verdict};
use crate::error::codes::library::UNSEASONED;
use crate::patience::REFRESH;
use crate::ports::service::{
    Holds, Item, ItemDetail, Medium, SeasonDetail, SeriesHeld, SERIES_MOST,
};
use crate::repair::{Attempt, Repair};

const SERVICE: &str = "media";
const HELD: &str = "/lemonfiber/media.serve/v1/series_held";
const REFRESHED: &str = "/lemonfiber/media.serve/v1/refresh";
const TITLED: &str = "/lemonfiber/media.serve/v1/title";
const BROKEN: &str = "0a1b2c3d4e5f60718293a4b5c6d7e8f9";

fn series(id: &str, title: &str, seasons: u32, episodes: u32) -> SeriesHeld {
    SeriesHeld {
        id: id.to_owned(),
        title: title.to_owned(),
        seasons,
        episodes,
    }
}

fn answered<T: serde::Serialize>(value: &T) -> Answer {
    Answer::reply(200, serde_json::to_string(value).unwrap_or_default())
}

fn title(seasons: Vec<SeasonDetail>) -> ItemDetail {
    ItemDetail {
        item: Item {
            id: BROKEN.to_owned(),
            title: "The Expanse".to_owned(),
            year: None,
            medium: Medium::Series,
            holds: Holds::default(),
        },
        overview: None,
        minutes: None,
        genres: Vec::new(),
        certificate: None,
        released: None,
        seasons,
    }
}

fn a_season() -> SeasonDetail {
    SeasonDetail {
        id: "s1".to_owned(),
        name: "Season 1".to_owned(),
        number: Some(1),
        episodes: Vec::new(),
    }
}

fn served(fake: &Arc<Fake>) -> SeasonsCheck {
    let adapter = Contracted::new(fake.clone(), "http://127.0.0.1:9", SERVICE, "key");
    SeasonsCheck::new(
        Some(SERVICE.to_owned()),
        Some(Arc::new(serve::Adapter(adapter))),
    )
}

fn holding(held: &[SeriesHeld]) -> Arc<Fake> {
    Fake::by_path(vec![(HELD, answered(&held))])
}

fn only(findings: Vec<Finding>) -> Finding {
    assert_eq!(findings.len(), 1, "{findings:?}");
    let nothing = Finding::in_category(Category::Config, "", "", Verdict::Pass { note: None });
    findings.into_iter().next().unwrap_or(nothing)
}

#[tokio::test]
async fn a_stack_with_nothing_serving_the_household_is_skipped() {
    let check = SeasonsCheck::new(None, None);
    let finding = only(check.run().await);
    assert_eq!(finding.check, CHECK);
    assert_eq!(finding.category, Category::Services);
    assert_eq!(finding.service, None);
    assert!(matches!(finding.verdict, Verdict::Skipped { .. }));
    assert!(check.writes_to(&repair_of(BROKEN)).is_empty());
}

#[tokio::test]
async fn a_media_server_that_cannot_be_reached_is_unverified_and_never_a_pass() {
    let check = SeasonsCheck::new(Some(SERVICE.to_owned()), None);
    let finding = only(check.run().await);
    assert_eq!(finding.check, CHECK);
    assert_eq!(finding.service.as_deref(), Some(SERVICE));
    assert!(matches!(
        finding.verdict,
        Verdict::Unverified { ref reason, .. } if reason.contains("could not be reached")
    ));
}

#[tokio::test]
async fn a_media_server_that_will_not_say_how_its_series_are_filed_is_unverified() {
    let finding = only(served(&Fake::silent()).run().await);
    assert!(matches!(
        finding.verdict,
        Verdict::Unverified { ref reason, .. } if reason.contains("how its series are filed")
    ));
}

#[tokio::test]
async fn every_series_opening_onto_its_seasons_passes_and_offers_nothing() {
    let check = served(&holding(&[
        series("a", "Severance", 2, 19),
        series("b", "Empty", 0, 0),
    ]));
    let found = check.run().await;
    assert_eq!(check.repairs(&found), Vec::new());
    let finding = only(found);
    assert_eq!(finding.check, CHECK);
    assert_eq!(
        finding.verdict,
        Verdict::Pass {
            note: Some(
                "each of the 2 series the media server holds opens onto its seasons".to_owned()
            )
        }
    );
}

#[tokio::test]
async fn a_library_with_no_series_says_so() {
    let finding = only(served(&holding(&[])).run().await);
    assert_eq!(
        finding.verdict,
        Verdict::Pass {
            note: Some("the media server holds no series".to_owned())
        }
    );
}

#[tokio::test]
async fn a_reading_cut_off_at_its_bound_says_it_read_only_the_first_of_them() {
    let held: Vec<SeriesHeld> = (0..SERIES_MOST)
        .map(|n| series(&format!("s{n}"), "Show", 1, 1))
        .collect();
    let finding = only(served(&holding(&held)).run().await);
    assert!(matches!(
        finding.verdict,
        Verdict::Pass { note: Some(ref note) }
            if note.contains(&format!("first {SERIES_MOST} series")) && note.contains("more than were read")
    ));
}

#[tokio::test]
async fn a_series_holding_episodes_and_no_seasons_is_a_warning_of_its_own() {
    let fake = holding(&[
        series(BROKEN, "The Expanse", 0, 12),
        series("b", "Severance", 2, 19),
        series("c", "Empty", 0, 0),
    ]);
    let finding = only(served(&fake).run().await);
    assert_eq!(finding.check, format!("{CHECK}.{BROKEN}"));
    assert_eq!(finding.title, "The seasons of The Expanse");
    assert_eq!(finding.service.as_deref(), Some(SERVICE));
    assert!(
        matches!(
            &finding.verdict,
            Verdict::Warn(problem) if problem.code == UNSEASONED
                && problem.summary
                    == "The Expanse holds 12 episodes and the media server answers no seasons for it"
        ),
        "{:?}",
        finding.verdict
    );
    assert!(fake
        .requests()
        .iter()
        .all(|request| request.url.ends_with(HELD)));
    assert!(fake
        .request()
        .and_then(|request| request.body)
        .is_some_and(|body| body.contains(&format!("\"most\":{SERIES_MOST}"))));
}

#[tokio::test]
async fn one_episode_is_said_as_one() {
    let finding = only(
        served(&holding(&[series(BROKEN, "Pilot", 0, 1)]))
            .run()
            .await,
    );
    assert!(matches!(
        finding.verdict,
        Verdict::Warn(ref problem) if problem.summary.starts_with("Pilot holds 1 episode and")
    ));
}

#[tokio::test]
async fn each_unseasoned_series_is_offered_a_refresh_of_its_own_that_cannot_be_undone() {
    let check = served(&holding(&[
        series(BROKEN, "The Expanse", 0, 12),
        series("f00d", "Dark", 0, 26),
    ]));
    let found = check.run().await;
    let repairs = check.repairs(&found);
    let checks: Vec<&str> = repairs.iter().map(|repair| repair.check.as_str()).collect();
    assert_eq!(
        checks,
        [format!("{CHECK}.{BROKEN}"), format!("{CHECK}.f00d")]
    );
    let first = repairs.first().cloned().unwrap_or(repair_of(""));
    assert_eq!(
        first.does,
        format!(
            "Have the media server read The Expanse afresh, and wait up to {} seconds for its \
             seasons",
            REFRESH.longest().as_secs()
        )
    );
    assert!(!first.reversible);
    assert_eq!(first.effects.len(), 1);
    assert_eq!(check.writes_to(&first), vec![SERVICE.to_owned()]);
}

#[tokio::test]
async fn a_finding_that_is_no_longer_a_warning_is_offered_nothing() {
    let check = served(&holding(&[series(BROKEN, "The Expanse", 0, 12)]));
    let mut found = check.run().await;
    for finding in &mut found {
        finding.verdict = Verdict::Pass { note: None };
    }
    assert_eq!(check.repairs(&found), Vec::new());
}

#[tokio::test]
async fn a_check_that_has_not_run_offers_nothing() {
    let check = served(&holding(&[series(BROKEN, "The Expanse", 0, 12)]));
    assert!(check.mender().is_some());
    assert_eq!(check.category(), Category::Services);
    assert_eq!(check.repairs(&[]), Vec::new());
}

fn repair_of(id: &str) -> Repair {
    Repair {
        check: format!("{CHECK}.{id}"),
        does: String::new(),
        effects: Vec::new(),
        reversible: false,
    }
}

fn refreshing(refresh: Answer, titles: Vec<Answer>) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (
            HELD,
            vec![answered(&[series(BROKEN, "The Expanse", 0, 12)])],
        ),
        (REFRESHED, vec![refresh]),
        (TITLED, titles),
    ])
}

#[tokio::test(start_paused = true)]
async fn a_refresh_is_asked_for_that_series_and_waited_on_until_it_opens() {
    let fake = refreshing(
        Answer::reply(204, ""),
        vec![
            answered(&Some(title(Vec::new()))),
            answered(&Some(title(vec![a_season()]))),
        ],
    );
    let check = served(&fake);
    check.run().await;
    let started = tokio::time::Instant::now();
    assert_eq!(check.mend(&repair_of(BROKEN)).await, Attempt::carried());
    assert_eq!(started.elapsed(), REFRESH.between * 2);
    let asked: Vec<(String, Option<String>)> = fake
        .requests()
        .into_iter()
        .skip(1)
        .map(|request| (request.url, request.body))
        .collect();
    let refresh = format!("{{\"id\":\"{BROKEN}\"}}");
    let titled = format!("{{\"member\":null,\"id\":\"{BROKEN}\"}}");
    assert_eq!(
        asked,
        vec![
            (format!("http://127.0.0.1:9{REFRESHED}"), Some(refresh)),
            (format!("http://127.0.0.1:9{TITLED}"), Some(titled.clone())),
            (format!("http://127.0.0.1:9{TITLED}"), Some(titled)),
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn a_series_still_shut_at_the_bound_is_left_for_the_check_to_judge() {
    let fake = refreshing(Answer::reply(204, ""), vec![Answer::Silent]);
    let check = served(&fake);
    check.run().await;
    let started = tokio::time::Instant::now();
    assert_eq!(check.mend(&repair_of(BROKEN)).await, Attempt::carried());
    assert_eq!(started.elapsed(), REFRESH.longest());
}

#[tokio::test]
async fn a_refresh_the_media_server_will_not_take_stops_with_nothing_changed() {
    let fake = refreshing(Answer::Silent, Vec::new());
    let check = served(&fake);
    check.run().await;
    assert!(matches!(
        check.mend(&repair_of(BROKEN)).await,
        Attempt::Stopped { ref leaving }
            if leaving.contains("read The Expanse afresh") && leaving.contains("nothing changed")
    ));
    assert!(!fake.asked_for(TITLED));
}

#[tokio::test]
async fn a_repair_naming_no_series_the_check_found_asks_nothing() {
    let fake = refreshing(Answer::reply(204, ""), Vec::new());
    let check = served(&fake);
    check.run().await;
    assert!(matches!(
        check.mend(&repair_of("elsewhere")).await,
        Attempt::Stopped { ref leaving } if leaving.contains("nothing was asked")
    ));
    assert!(!fake.asked_for(REFRESHED));
}

#[tokio::test]
async fn a_media_server_that_was_never_asked_is_asked_for_no_refresh() {
    let check = SeasonsCheck::new(Some(SERVICE.to_owned()), None);
    check.run().await;
    assert!(matches!(
        check.mend(&repair_of(BROKEN)).await,
        Attempt::Stopped { .. }
    ));
}
