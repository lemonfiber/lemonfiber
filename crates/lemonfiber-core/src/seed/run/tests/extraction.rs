//! Telling an extractor speaking its contract whose completed downloads it unpacks.

use std::collections::{BTreeMap, BTreeSet};

use super::*;

/// Where an extractor is told whom to watch.
const WATCH: &str = "/lemonfiber/archive.extract/v1/watch";

/// A plugin `unpacked` whose service `extractor` speaks `archive.extract` where
/// `speaking` and asks every curator.
fn the_extractor(speaking: bool) -> crate::plugin::Installed {
    let mut installed = crate::test_support::contracted("unpacked", "extractor", "archive.extract");
    for placed in &mut installed.services {
        placed.asks = vec![crate::plugin::Asking {
            native: None,
            capability: "library.curate".to_owned(),
            each: true,
        }];
        if !speaking {
            placed.speaks.clear();
        }
    }
    installed
}

/// [`the_extractor`] beside a curator of television, of films and of music, each moved
/// into its plugin.
fn an_extractor(project: &std::path::Path, speaking: bool) -> crate::wiring::Fillers {
    let curators = vec![
        curator("sonarr", 8989, "tv"),
        curator("radarr", 7878, "movies"),
        curator("lidarr", 8686, "music"),
    ];
    let installed: Vec<crate::plugin::Installed> = [the_extractor(speaking)]
        .into_iter()
        .chain(curators_moved(&curators))
        .collect();
    fillers_trusting(
        curators,
        &installed,
        project,
        &beside_the_bundle(&first_party("unpacked")),
    )
}

/// An extractor answering every `watch` with `answer`.
fn watching(answer: Answer) -> Arc<Fake> {
    Fake::by_path(vec![(WATCH, answer)])
}

/// Every curator each `watch` carried, as media type and folder.
fn watched(http: &Fake) -> Vec<Vec<(String, String)>> {
    #[derive(serde::Deserialize)]
    struct Told {
        curators: Vec<crate::ports::service::Curated>,
    }
    http.requests()
        .into_iter()
        .filter(|one| one.url.ends_with(WATCH))
        .filter_map(|one| serde_json::from_str::<Told>(one.body.as_deref()?).ok())
        .map(|told| {
            let mut curators: Vec<(String, String)> = told
                .curators
                .into_iter()
                .map(|one| (one.media_type, one.folder))
                .collect();
            curators.sort();
            curators
        })
        .collect()
}

/// What an extractor answers a call its upstream answered by refusing.
fn refused() -> Answer {
    let kind = lemonfiber_contract::wire::Kind::Refused;
    Answer::served(
        kind.status(),
        lemonfiber_contract::wire::PROBLEM,
        serde_json::to_string(&lemonfiber_contract::wire::Refusal::new(kind, "refused"))
            .unwrap_or_default(),
    )
}

/// The folders the clients read back for each category.
fn held(folders: &[(&str, &[&str])]) -> BTreeMap<String, BTreeSet<String>> {
    folders
        .iter()
        .map(|(category, held)| {
            (
                (*category).to_owned(),
                held.iter().map(|one| (*one).to_owned()).collect(),
            )
        })
        .collect()
}

#[tokio::test]
async fn every_curator_is_handed_at_once_with_the_folder_its_downloads_complete_into() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("extract-contracted");
    let http = watching(Answer::reply(204, ""));
    let ctx = contracted_ctx(&project, "extractor", true, http.clone());

    let wirings = super::super::extraction::seed_extraction(
        &ctx,
        &an_extractor(&project, true),
        &BTreeMap::new(),
    )
    .await;

    assert_eq!(
        wirings
            .iter()
            .map(|wiring| &wiring.state)
            .collect::<Vec<_>>(),
        [&crate::seed::State::Wired],
        "{wirings:?}"
    );
    assert_eq!(
        watched(&http),
        [vec![
            (
                "movies".to_owned(),
                "/data/downloads/complete/movies".to_owned()
            ),
            (
                "music".to_owned(),
                "/data/downloads/complete/music".to_owned()
            ),
            ("tv".to_owned(), "/data/downloads/complete/tv".to_owned()),
        ]]
    );
}

#[tokio::test]
async fn a_curator_is_handed_the_folder_the_clients_read_back() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("extract-read-back");
    let http = watching(Answer::reply(204, ""));
    let ctx = contracted_ctx(&project, "extractor", true, http.clone());

    let _ = super::super::extraction::seed_extraction(
        &ctx,
        &an_extractor(&project, true),
        &held(&[
            ("tv", &["/data/elsewhere/tv"]),
            (
                "movies",
                &["/data/downloads/complete/movies", "/data/elsewhere/films"],
            ),
        ]),
    )
    .await;

    assert_eq!(
        watched(&http),
        [vec![
            (
                "movies".to_owned(),
                "/data/downloads/complete/movies".to_owned()
            ),
            (
                "music".to_owned(),
                "/data/downloads/complete/music".to_owned()
            ),
            ("tv".to_owned(), "/data/elsewhere/tv".to_owned()),
        ]]
    );
}

#[tokio::test]
async fn a_rehearsal_says_whom_it_would_hand_and_hands_nobody() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("extract-rehearsed");
    let http = watching(Answer::reply(204, ""));
    let ctx = contracted_ctx(&project, "extractor", true, http.clone()).rehearsing();

    let wirings = super::super::extraction::seed_extraction(
        &ctx,
        &an_extractor(&project, true),
        &BTreeMap::new(),
    )
    .await;

    assert!(
        matches!(
            wirings.first().map(|wiring| &wiring.state),
            Some(crate::seed::State::WouldWire { ours: Some(ours), .. })
                if ours.contains("tv in /data/downloads/complete/tv")
        ),
        "{wirings:?}"
    );
    assert!(watched(&http).is_empty());
}

#[tokio::test]
async fn an_extractor_speaking_no_contract_is_told_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("extract-unspoken");
    let http = watching(Answer::reply(204, ""));
    let ctx = contracted_ctx(&project, "extractor", true, http.clone());

    let wirings = super::super::extraction::seed_extraction(
        &ctx,
        &an_extractor(&project, false),
        &BTreeMap::new(),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn an_extractor_that_refuses_is_not_reported_as_watching() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("extract-refused");
    let ctx = contracted_ctx(&project, "extractor", true, watching(refused()));

    let wirings = super::super::extraction::seed_extraction(
        &ctx,
        &an_extractor(&project, true),
        &BTreeMap::new(),
    )
    .await;

    assert!(
        wirings
            .iter()
            .all(|wiring| !matches!(wiring.state, crate::seed::State::Wired)),
        "{wirings:?}"
    );
}

#[tokio::test]
async fn a_curator_holding_no_minted_key_holds_the_whole_watch_back() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("extract-unkeyed");
    let http = watching(Answer::reply(204, ""));
    let ctx = forgetting_minted(contracted_ctx(&project, "extractor", true, http.clone()));

    let wirings = super::super::extraction::seed_extraction(
        &ctx,
        &an_extractor(&project, true),
        &BTreeMap::new(),
    )
    .await;

    assert!(
        matches!(
            wirings.first().map(|wiring| &wiring.state),
            Some(crate::seed::State::Skipped { .. })
        ),
        "{wirings:?}"
    );
    assert!(watched(&http).is_empty());
}

#[tokio::test]
async fn an_extractor_lemonfiber_cannot_reach_over_its_contract_is_told_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("extract-unreached");
    let http = watching(Answer::reply(204, ""));
    let ctx = contracted_ctx(&project, "extractor", false, http.clone());

    let wirings = super::super::extraction::seed_extraction(
        &ctx,
        &an_extractor(&project, true),
        &BTreeMap::new(),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn a_curator_filing_media_no_category_is_filed_under_is_not_handed() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("extract-uncategorised");
    let http = watching(Answer::reply(204, ""));
    let ctx = contracted_ctx(&project, "extractor", true, http.clone());
    let mut comics = crate::test_support::a_placed("comics", &["library.curate"], Some(8787));
    comics.media_types = vec!["comics".to_owned()];
    comics.native = Some("servarr".to_owned());
    let mut shelf = crate::test_support::an_installed("comics", vec![comics]);
    shelf.manifest = crate::test_support::CONTRACTED_MANIFEST.to_owned();
    shelf.tree = crate::test_support::A_TREE.to_owned();
    let curators = vec![curator("sonarr", 8989, "tv")];
    let installed: Vec<crate::plugin::Installed> = [the_extractor(true), shelf]
        .into_iter()
        .chain(curators_moved(&curators))
        .collect();
    let trusted: Vec<crate::plugin::first_party::FirstParty> = first_party("unpacked")
        .into_iter()
        .chain(first_party("comics"))
        .collect();
    let fillers = fillers_trusting(curators, &installed, &project, &beside_the_bundle(&trusted));

    let wirings = super::super::extraction::seed_extraction(&ctx, &fillers, &BTreeMap::new()).await;

    assert_eq!(
        wirings
            .iter()
            .map(|wiring| &wiring.state)
            .collect::<Vec<_>>(),
        [&crate::seed::State::Wired],
        "{wirings:?}"
    );
    assert_eq!(
        watched(&http),
        [vec![(
            "tv".to_owned(),
            "/data/downloads/complete/tv".to_owned()
        )]]
    );
}
