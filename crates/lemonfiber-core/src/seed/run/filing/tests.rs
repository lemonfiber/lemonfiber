use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use async_trait::async_trait;

use super::{categories, completed, field, filed};
use crate::baseline::Baseline;
use crate::ports::service::{Failure, Filed, Filing};
use crate::seed::State;
use crate::test_support::a_context;

/// The id the client is recorded under.
const CLIENT: &str = "sabnzbd";

/// A client holding each category's folder in memory, answering a write with `refusing`
/// where it is set, and filing nothing it is told where `ignoring`.
#[derive(Default)]
struct Holding {
    held: Mutex<BTreeMap<String, String>>,
    told: Mutex<Vec<Filed>>,
    refusing: bool,
    ignoring: bool,
    unread: bool,
}

impl Holding {
    fn with(held: &[(&str, &str)]) -> Self {
        Self {
            held: Mutex::new(
                held.iter()
                    .map(|(category, folder)| ((*category).to_owned(), (*folder).to_owned()))
                    .collect(),
            ),
            ..Self::default()
        }
    }

    fn told(&self) -> Vec<Filed> {
        self.told
            .lock()
            .map(|told| told.clone())
            .unwrap_or_default()
    }

    fn holds(&self, category: &str) -> Option<String> {
        self.held
            .lock()
            .ok()
            .and_then(|held| held.get(category).cloned())
    }
}

#[async_trait]
impl Filing for Holding {
    async fn filed(&self) -> Result<Vec<Filed>, Failure> {
        if self.unread {
            return Err(Failure::Unavailable {
                service: CLIENT.to_owned(),
            });
        }
        Ok(self
            .held
            .lock()
            .map(|held| {
                held.iter()
                    .map(|(category, folder)| Filed {
                        category: category.clone(),
                        folder: folder.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn file_under(&self, folders: &[Filed]) -> Result<(), Failure> {
        if let Ok(mut told) = self.told.lock() {
            told.extend_from_slice(folders);
        }
        if self.refusing {
            return Err(Failure::Refused {
                service: CLIENT.to_owned(),
                detail: "no".to_owned(),
            });
        }
        if !self.ignoring {
            if let Ok(mut held) = self.held.lock() {
                for one in folders {
                    held.insert(one.category.clone(), one.folder.clone());
                }
            }
        }
        Ok(())
    }
}

/// Television and films, as two curators hand a client them.
fn tv_and_movies() -> BTreeSet<String> {
    ["tv", "movies"].into_iter().map(str::to_owned).collect()
}

/// A baseline recording that lemonfiber filed `category` where it files it.
fn filed_before(category: &str) -> Baseline {
    let mut baseline = Baseline::new();
    baseline.record(CLIENT, &field(category), &completed(category), "earlier");
    baseline
}

#[tokio::test]
async fn a_client_holding_its_own_defaults_is_told_the_shared_folders_and_they_are_recorded() {
    let client = Holding::with(&[("tv", "/data/downloads/usenet/complete/tv")]);
    let mut baseline = Baseline::new();

    let state = filed(
        &a_context().build(),
        &client,
        CLIENT,
        &tv_and_movies(),
        &mut baseline,
    )
    .await
    .0;

    assert_eq!(state, State::Wired);
    assert_eq!(client.holds("tv"), Some(completed("tv")));
    assert_eq!(client.holds("movies"), Some(completed("movies")));
    assert_eq!(
        baseline.expected(CLIENT, &field("movies")),
        Some(completed("movies").as_str())
    );
}

#[tokio::test]
async fn a_client_already_filing_both_is_told_nothing() {
    let client = Holding::with(&[("tv", &completed("tv")), ("movies", &completed("movies"))]);

    let state = filed(
        &a_context().build(),
        &client,
        CLIENT,
        &tv_and_movies(),
        &mut Baseline::new(),
    )
    .await
    .0;

    assert_eq!(state, State::AlreadyWired);
    assert!(client.told().is_empty());
}

#[tokio::test]
async fn a_folder_the_operator_moved_after_lemonfiber_filed_it_is_kept_as_drift() {
    let client = Holding::with(&[("tv", "/data/elsewhere"), ("movies", &completed("movies"))]);
    let mut baseline = filed_before("tv");

    let state = filed(
        &a_context().build(),
        &client,
        CLIENT,
        &tv_and_movies(),
        &mut baseline,
    )
    .await
    .0;

    assert_eq!(state, State::Drifted);
    assert!(client.told().is_empty());
    assert_eq!(client.holds("tv").as_deref(), Some("/data/elsewhere"));
    assert_eq!(
        baseline.expected(CLIENT, &field("tv")),
        Some(completed("tv").as_str())
    );
}

#[tokio::test]
async fn a_category_gone_from_the_client_is_filed_again() {
    let client = Holding::with(&[("movies", &completed("movies"))]);

    let state = filed(
        &a_context().build(),
        &client,
        CLIENT,
        &tv_and_movies(),
        &mut filed_before("tv"),
    )
    .await
    .0;

    assert_eq!(state, State::Wired);
    assert_eq!(
        client.told(),
        [Filed {
            category: "tv".to_owned(),
            folder: completed("tv"),
        }]
    );
}

#[tokio::test]
async fn a_rehearsal_says_what_it_would_file_and_files_nothing() {
    let client = Holding::with(&[("tv", "/data/downloads/usenet/complete/tv")]);
    let mut baseline = Baseline::new();

    let state = filed(
        &a_context().build().rehearsing(),
        &client,
        CLIENT,
        &tv_and_movies(),
        &mut baseline,
    )
    .await
    .0;

    assert_eq!(
        state,
        State::WouldWire {
            yours: Some(
                "movies under nothing, tv under /data/downloads/usenet/complete/tv".to_owned()
            ),
            ours: Some(format!(
                "movies under {}, tv under {}",
                completed("movies"),
                completed("tv")
            )),
        }
    );
    assert!(client.told().is_empty());
    assert!(baseline.expected(CLIENT, &field("tv")).is_none());
}

#[tokio::test]
async fn a_client_that_refuses_is_said_and_nothing_is_recorded() {
    let client = Holding {
        refusing: true,
        ..Holding::default()
    };
    let mut baseline = Baseline::new();

    let state = filed(
        &a_context().build(),
        &client,
        CLIENT,
        &tv_and_movies(),
        &mut baseline,
    )
    .await
    .0;

    assert!(
        matches!(&state, State::Failed { detail } if detail.contains("could not be set")),
        "{state:?}"
    );
    assert!(baseline.expected(CLIENT, &field("tv")).is_none());
}

#[tokio::test]
async fn a_client_that_takes_a_folder_and_files_elsewhere_is_said_as_that() {
    let client = Holding {
        ignoring: true,
        ..Holding::with(&[("tv", "/data/downloads/usenet/complete/tv")])
    };

    let state = filed(
        &a_context().build(),
        &client,
        CLIENT,
        &tv_and_movies(),
        &mut Baseline::new(),
    )
    .await
    .0;

    assert!(
        matches!(&state, State::Failed { detail }
            if detail.contains("files it under")),
        "{state:?}"
    );
}

#[tokio::test]
async fn a_client_that_will_not_say_what_it_files_is_told_nothing() {
    let client = Holding {
        unread: true,
        ..Holding::default()
    };

    let state = filed(
        &a_context().build(),
        &client,
        CLIENT,
        &tv_and_movies(),
        &mut Baseline::new(),
    )
    .await
    .0;

    assert!(
        !matches!(state, State::Wired | State::AlreadyWired),
        "{state:?}"
    );
    assert!(client.told().is_empty());
}

#[test]
fn each_client_a_curator_hands_downloads_to_is_given_that_curators_category() {
    let Ok(manifest) = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK) else {
        unreachable!("the shipped stack reads")
    };
    let fillers = crate::test_support::stack_fillers(
        manifest.services,
        &[],
        None,
        crate::plugin::first_party::EMBEDDED,
    );

    let wanted = categories(&fillers);

    assert!(!wanted.is_empty(), "no client was handed a category");
    for (client, categories) in &wanted {
        assert!(
            fillers
                .service(client)
                .is_some_and(|filler| filler.media_types.is_empty()),
            "{client} is not a download client"
        );
        assert!(categories.contains("tv"), "{client}: {categories:?}");
    }
}

#[test]
fn a_category_completes_into_the_one_folder_both_clients_share() {
    assert_eq!(completed("tv"), "/data/downloads/complete/tv");
}

/// The shipped stack's torrent client, its password recorded, holding no category until
/// it is told and then every one it was told, and managing torrents by hand until it is
/// switched over.
#[tokio::test]
async fn the_stacks_torrent_client_is_told_and_read_back_through_its_adapter() {
    use lemonfiber_fixtures::http::{Answer, Fake};

    let Ok(manifest) = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK) else {
        unreachable!("the shipped stack reads")
    };
    let fillers = crate::test_support::stack_fillers(
        manifest.services,
        &[],
        None,
        crate::plugin::first_party::EMBEDDED,
    );
    let wanted = categories(&fillers);
    let Some((client, handed)) = wanted.iter().find(|(id, _)| {
        fillers
            .service(id)
            .is_some_and(|filler| filler.speaks(lemonfiber_manifest::ApiKind::Qbittorrent))
    }) else {
        unreachable!("the shipped stack hands its torrent client a category")
    };
    let filed_now: BTreeMap<&str, serde_json::Value> = handed
        .iter()
        .map(|category| {
            (
                category.as_str(),
                serde_json::json!({ "savePath": completed(category) }),
            )
        })
        .collect();
    let by_hand = r#"{"save_path":"/data/downloads/torrents/complete","auto_tmm_enabled":false}"#;
    let http = Fake::by_path_in_turn(vec![
        ("auth/login", vec![Answer::reply(200, "Ok.")]),
        (
            "torrents/categories",
            vec![
                Answer::reply(200, "{}"),
                Answer::reply(200, "{}"),
                Answer::reply(200, serde_json::json!(filed_now).to_string()),
            ],
        ),
        (
            "app/preferences",
            vec![
                Answer::reply(200, by_hand),
                Answer::reply(200, by_hand),
                Answer::reply(
                    200,
                    r#"{"save_path":"/data/downloads/torrents/complete","auto_tmm_enabled":true}"#,
                ),
            ],
        ),
        ("Category", vec![Answer::reply(200, "")]),
        ("app/setPreferences", vec![Answer::reply(200, "")]),
    ]);
    let ctx = a_context()
        .settings(crate::config::Settings {
            env_file: Some(crate::test_support::env_at(
                "filing-torrent",
                &crate::test_support::a_password(),
            )),
            ..crate::config::Settings::default()
        })
        .build()
        .with_http(http.clone());
    let mut baseline = Baseline::new();

    let (wirings, completed_now) =
        super::seed_completed_folders(&ctx, &fillers, &mut baseline).await;

    let torrent = wirings
        .iter()
        .find(|wiring| wiring.connection.starts_with("qBittorrent"));
    assert_eq!(
        torrent.map(|wiring| (&wiring.connection, &wiring.state)),
        Some((&"qBittorrent completed folders".to_owned(), &State::Wired)),
        "{wirings:?}"
    );
    assert!(handed.iter().all(|category| baseline
        .expected(client, &field(category))
        .is_some_and(|recorded| recorded == completed(category))));
    assert!(http
        .requests()
        .iter()
        .any(|asked| asked.url.ends_with("/torrents/createCategory")));
    assert!(handed.iter().all(|category| completed_now
        .get(category)
        .is_some_and(|folders| folders.contains(&completed(category)))));
}
