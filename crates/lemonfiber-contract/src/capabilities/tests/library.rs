//! `library.curate` across its contract: one script run against a curator in process and
//! against the same curator through the contract, which must answer alike and be told
//! alike: a failure is named after the service the core called, as the core names it.

use lemonfiber_fixtures::upstream::library::FINGERPRINT;
use lemonfiber_ports::media::{Format, Kind, TitleKind};
use lemonfiber_ports::service::{
    AddPlan, Carried, Carrying, Catalogue, CatalogueEntry, Category, Credential, DownloadClient,
    Fulfilling, Importing, Maintenance, MusicQuality, Pipeline, Protocol, QualityReleases, Queues,
    Record, RootFolder, TitleRequest,
};
use lemonfiber_ports::Client;

use super::{crosses_alike, curate};

/// Every operation of the capability, once, each answer written down.
async fn script<C>(curator: &C) -> Vec<String>
where
    C: Client
        + Maintenance
        + Importing
        + Carrying
        + Catalogue
        + Queues
        + QualityReleases
        + MusicQuality
        + Pipeline
        + Fulfilling,
{
    let client = DownloadClient {
        name: "fetcher".to_owned(),
        host: "fetcher".to_owned(),
        port: 6789,
        protocol: Protocol("sabnzbd".to_owned()),
        credential: Credential::UserPass {
            username: "u".to_owned(),
            password: "p".to_owned(),
        },
        category: Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        },
    };
    let folder = RootFolder {
        path: "/films".to_owned(),
        media_type: "movies".to_owned(),
    };
    let carried = Carried {
        name: "an indexer".to_owned(),
        profile: Some("HD".to_owned()),
        folder: None,
        rest: r#"{"enable":true}"#.to_owned(),
    };
    let entry = CatalogueEntry {
        title: "The Matrix".to_owned(),
        year: Some(1999),
        reference: 603,
        held_as: Some(7),
    };
    let plan = AddPlan {
        root_folder: "/films".to_owned(),
        quality_profile: 4,
    };
    let request = TitleRequest {
        reference: 73244,
        root_folder: "/tv".to_owned(),
        quality_profile: 4,
        seasons: vec![1, 2],
        anime: false,
        later_seasons: true,
        search: true,
    };
    vec![
        format!("{:?}", curator.identity().await),
        format!("{:?}", curator.register_download_client(&client).await),
        format!("{:?}", curator.update_download_client("2", &client).await),
        format!(
            "{:?}",
            curator.set_client_field("2", "priority", None).await
        ),
        format!(
            "{:?}",
            curator.set_client_field("2", "category", Some("tv")).await
        ),
        format!("{:?}", curator.test_download_clients().await),
        format!("{:?}", curator.register_root_folder(&folder).await),
        format!("{:?}", curator.root_folders().await),
        format!("{:?}", curator.download_clients().await),
        format!("{:?}", curator.quality_profiles().await),
        format!("{:?}", curator.search_upgrades(Kind::Tv).await),
        format!("{:?}", curator.wanting().await),
        format!("{:?}", curator.hardlinks().await),
        format!("{:?}", curator.set_hardlinks(false).await),
        format!("{:?}", curator.records(Record::Indexer).await),
        format!("{:?}", curator.carry(Record::Film, &carried).await),
        format!("{:?}", curator.lookup(Kind::Movies, "matrix").await),
        format!("{:?}", curator.add_plan(Kind::Tv).await),
        format!("{:?}", curator.add(Kind::Movies, &entry, &plan).await),
        format!("{:?}", curator.indexer_count().await),
        format!("{:?}", curator.queue().await),
        format!("{:?}", curator.probe_releases(TitleKind::Tv).await),
        format!("{:?}", curator.probe_releases(TitleKind::Movies).await),
        format!("{:?}", curator.apply_music_format(Format::Lossless).await),
        format!("{:?}", curator.library(TitleKind::Movies).await),
        format!("{:?}", curator.find_items(TitleKind::Tv, "office").await),
        format!("{:?}", curator.item_history(TitleKind::Tv, 12).await),
        format!("{:?}", curator.item_queue(TitleKind::Movies, 13).await),
        format!("{:?}", curator.item_parts(TitleKind::Tv, 14, Some(2)).await),
        format!("{:?}", curator.item_parts(TitleKind::Tv, 14, None).await),
        format!("{:?}", curator.stuck_items(TitleKind::Movies).await),
        format!("{:?}", curator.catalogued(Kind::Tv, 73244).await),
        format!("{:?}", curator.catalogued(Kind::Movies, 0).await),
        format!("{:?}", curator.titles(TitleKind::Tv).await),
        format!("{:?}", curator.held_title(Kind::Movies, 3).await),
        format!("{:?}", curator.request(Kind::Tv, &request).await),
        format!("{:?}", curator.search(TitleKind::Tv, 9).await),
        format!("{:?}", curator.remove(Kind::Movies, 0).await),
        format!("{:?}", curator.downloading(TitleKind::Tv).await),
        format!("{:?}", curator.stuck_items(TitleKind::Tv).await),
    ]
    .into_iter()
    .chain(albums_and_books(curator).await)
    .collect()
}

/// Dropping and grabbing again, then each operation taking music and books asked of one of
/// them, each answer written down.
async fn albums_and_books<C: QualityReleases + Pipeline + Fulfilling>(curator: &C) -> Vec<String> {
    vec![
        format!("{:?}", curator.unqueue(TitleKind::Tv, 41, true).await),
        format!("{:?}", curator.unqueue(TitleKind::Movies, 0, false).await),
        format!("{:?}", curator.grab(TitleKind::Tv, 9, FINGERPRINT).await),
        format!(
            "{:?}",
            curator.grab(TitleKind::Movies, 7, "elsewhere").await
        ),
        format!("{:?}", curator.probe_releases(TitleKind::Music).await),
        format!("{:?}", curator.probe_releases(TitleKind::Books).await),
        format!("{:?}", curator.library(TitleKind::Music).await),
        format!("{:?}", curator.find_items(TitleKind::Books, "dune").await),
        format!("{:?}", curator.item_history(TitleKind::Music, 21).await),
        format!("{:?}", curator.item_queue(TitleKind::Books, 22).await),
        format!("{:?}", curator.item_parts(TitleKind::Music, 21, None).await),
        format!("{:?}", curator.stuck_items(TitleKind::Books).await),
        format!("{:?}", curator.titles(TitleKind::Music).await),
        format!("{:?}", curator.search(TitleKind::Books, 22).await),
        format!("{:?}", curator.downloading(TitleKind::Music).await),
        format!("{:?}", curator.unqueue(TitleKind::Books, 42, true).await),
        format!(
            "{:?}",
            curator.grab(TitleKind::Music, 21, FINGERPRINT).await
        ),
    ]
}

#[tokio::test]
async fn a_curator_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        script,
        curate::Adapter,
        [
            "register",
            "update",
            "set",
            "set",
            "folder",
            "upgrades",
            "hardlinks",
            "carry",
            "add",
            "format",
            "library",
            "find",
            "history",
            "queued",
            "parts",
            "parts",
            "stuck",
            "catalogued",
            "catalogued",
            "titles",
            "held",
            "request",
            "search",
            "remove",
            "downloading",
            "stuck",
            "unqueue",
            "unqueue",
            "grab",
            "grab",
            "library",
            "find",
            "history",
            "queued",
            "parts",
            "stuck",
            "titles",
            "search",
            "downloading",
            "unqueue",
            "grab"
        ]
    );
}

#[test]
fn the_client_the_cases_register_answers_a_curators_tests_and_files_under_each_category() {
    let answering =
        crate::capabilities::library::client_answering(&["tv".to_owned(), "movies".to_owned()]);
    let said = |mode: &str| {
        answering
            .answers
            .iter()
            .find(|said| {
                said.method == "GET"
                    && said.path == "/api"
                    && said.query == Some(("mode".to_owned(), mode.to_owned()))
            })
            .and_then(|said| serde_json::from_str::<serde_json::Value>(said.body.as_deref()?).ok())
    };
    let categories: Vec<String> = said("get_config")
        .and_then(|config| config.pointer("/config/categories")?.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|one| one.get("name")?.as_str().map(str::to_owned))
        .collect();

    assert_eq!(answering.port, 8080);
    assert_eq!(categories, vec!["*", "conformance", "tv", "movies"]);
    assert!(said("version").is_some_and(|version| version
        .get("version")
        .is_some_and(serde_json::Value::is_string)));
    assert!(said("queue").is_some() && said("history").is_some());
    assert_eq!(
        said("get_cats").and_then(|cats| cats.get("categories").cloned()),
        Some(serde_json::json!(["*", "conformance", "tv", "movies"]))
    );
    assert_eq!(answering.answers.len(), 5);
}
