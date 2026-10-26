use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_fixtures::http::{Answer, Fake};

use super::{categories, completed, field, filed};
use crate::baseline::Baseline;
use crate::ports::service::{Failure, Filed, Filing};
use crate::seed::State;
use crate::test_support::{a_context, torrent};

/// The id the client is recorded under.
const CLIENT: &str = "sabnzbd";

/// A client holding each category's folder in memory, answering a write with `refusing`
/// where it is set, filing nothing it is told where `ignoring`, and answering only as many
/// reads as `answering` says where it says.
#[derive(Default)]
struct Holding {
    held: Mutex<BTreeMap<String, String>>,
    told: Mutex<Vec<Filed>>,
    reads: Mutex<usize>,
    refusing: bool,
    ignoring: bool,
    answering: Option<usize>,
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
        let read = self
            .reads
            .lock()
            .map(|mut reads| {
                *reads += 1;
                *reads
            })
            .unwrap_or_default();
        if self.answering.is_some_and(|answered| read > answered) {
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

/// The shipped stack's services.
fn shipped_services() -> Vec<lemonfiber_manifest::Service> {
    let Ok(manifest) = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK) else {
        unreachable!("the shipped stack reads")
    };
    manifest.services
}

/// Every curator moved into its plugin, with each of `beside`.
fn moved_beside(
    beside: impl IntoIterator<Item = Option<crate::plugin::Installed>>,
) -> Vec<crate::plugin::Installed> {
    crate::test_support::curators::ALL
        .iter()
        .map(|curator| curator.installed())
        .chain(beside)
        .flatten()
        .collect()
}

/// What `services` come to, asked for as the shipped stack asks, every curator and both
/// download clients moved into their plugins.
fn shipped(services: Vec<lemonfiber_manifest::Service>) -> crate::wiring::Fillers {
    crate::test_support::stack_fillers(
        services,
        &moved_beside([torrent::moved(), crate::test_support::usenet::moved()]),
        None,
        crate::plugin::first_party::EMBEDDED,
    )
}

/// A context reaching the shipped stack's torrent client moved into its plugin, over
/// `http`, with the stack's fillers as it is written beside `installed`.
fn reaching_the_torrent_client(
    name: &str,
    services: Vec<lemonfiber_manifest::Service>,
    installed: &[crate::plugin::Installed],
    http: Arc<Fake>,
) -> (crate::app::Ctx, crate::wiring::Fillers) {
    let ctx = torrent::reaching(
        a_context()
            .settings(crate::config::Settings {
                env_file: Some(crate::test_support::env_without_password(name)),
                ..crate::config::Settings::default()
            })
            .build()
            .with_http(http),
    );
    let project = match ctx.stack {
        crate::stack::Source::External(project) => Some(project),
        crate::stack::Source::Embedded(_) => None,
    };
    let fillers = crate::test_support::stack_fillers(
        services,
        installed,
        project,
        crate::plugin::first_party::EMBEDDED,
    );
    (ctx, fillers)
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
        answering: Some(0),
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

#[tokio::test]
async fn a_client_that_files_and_then_will_not_say_where_is_said_and_nothing_is_recorded() {
    let client = Holding {
        answering: Some(1),
        ..Holding::with(&[("tv", "/data/downloads/usenet/complete/tv")])
    };
    let mut baseline = Baseline::new();

    let (state, now) = filed(
        &a_context().build(),
        &client,
        CLIENT,
        &tv_and_movies(),
        &mut baseline,
    )
    .await;

    assert!(
        matches!(&state, State::Failed { detail } if detail.contains("could not be read back")),
        "{state:?}"
    );
    assert!(now.is_empty(), "{now:?}");
    assert!(baseline.expected(CLIENT, &field("movies")).is_none());
}

#[tokio::test]
async fn a_moved_folder_stays_drift_while_a_missing_one_is_filed_beside_it() {
    let client = Holding::with(&[("tv", "/data/elsewhere")]);
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
    assert_eq!(
        client.told(),
        [Filed {
            category: "movies".to_owned(),
            folder: completed("movies"),
        }]
    );
    assert_eq!(client.holds("tv").as_deref(), Some("/data/elsewhere"));
    assert_eq!(
        baseline.expected(CLIENT, &field("movies")),
        Some(completed("movies").as_str())
    );
}

#[test]
fn each_client_a_curator_hands_downloads_to_is_given_that_curators_category() {
    let fillers = shipped(shipped_services());

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

/// The shipped stack's torrent client, moved into its plugin, holding no category until
/// it is told and then every one it was told, over its contract.
#[tokio::test]
async fn the_moved_torrent_client_is_told_and_read_back_over_its_contract() {
    let filed_now: Vec<Filed> = ["movies", "music", "tv"]
        .into_iter()
        .map(|category| Filed {
            category: category.to_owned(),
            folder: completed(category),
        })
        .collect();
    let http = Fake::by_path_in_turn(vec![
        (
            torrent::operation("filed"),
            vec![
                torrent::answered(&Vec::<Filed>::new()),
                torrent::answered(&filed_now),
            ],
        ),
        (torrent::operation("file_under"), vec![torrent::taken()]),
    ]);
    let (ctx, fillers) = reaching_the_torrent_client(
        "filing-torrent",
        shipped_services(),
        &moved_beside([torrent::moved()]),
        http.clone(),
    );
    let wanted = categories(&fillers);
    let handed = wanted.get(torrent::ADAPTER).cloned().unwrap_or_default();
    let mut baseline = Baseline::new();

    let (wirings, completed_now) =
        super::seed_completed_folders(&ctx, &fillers, &mut baseline).await;

    assert!(!handed.is_empty(), "{wanted:?}");
    let told = wirings
        .iter()
        .find(|wiring| wiring.connection.starts_with("qBittorrent"));
    assert_eq!(
        told.map(|wiring| (&wiring.connection, &wiring.state)),
        Some((&"qBittorrent completed folders".to_owned(), &State::Wired)),
        "{wirings:?}"
    );
    assert!(handed.iter().all(|category| baseline
        .expected(torrent::ADAPTER, &field(category))
        .is_some_and(|recorded| recorded == completed(category))));
    assert!(http.asked_for(torrent::operation("file_under")));
    assert!(handed.iter().all(|category| completed_now
        .get(category)
        .is_some_and(|folders| folders.contains(&completed(category)))));
}

#[tokio::test]
async fn a_client_no_curator_hands_a_category_is_told_nothing() {
    let mut installed = moved_beside([torrent::moved()]);
    for placed in installed
        .iter_mut()
        .flat_map(|plugin| plugin.services.iter_mut())
        .filter(|placed| !placed.media_types.is_empty())
    {
        placed.media_types = vec!["comics".to_owned()];
    }
    let http = Fake::always(Answer::reply(200, "{}"));
    let (ctx, fillers) = reaching_the_torrent_client(
        "filing-uncategorised",
        shipped_services(),
        &installed,
        http.clone(),
    );

    let (wirings, completed_now) =
        super::seed_completed_folders(&ctx, &fillers, &mut Baseline::new()).await;

    assert!(categories(&fillers).is_empty());
    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(completed_now.is_empty(), "{completed_now:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}
