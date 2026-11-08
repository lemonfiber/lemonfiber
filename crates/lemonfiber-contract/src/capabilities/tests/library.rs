//! `library.curate` across its contract: one script run against a curator in process and
//! against the same curator through the contract, which must answer alike and be told
//! alike: a failure is named after the service the core called, as the core names it.

use async_trait::async_trait;
use lemonfiber_ports::media::{Format, Kind};
use lemonfiber_ports::service::stage::Stage;
use lemonfiber_ports::service::{
    AddPlan, Added, Carried, Carrying, Catalogue, CatalogueEntry, Category, ClientProbe,
    Credential, CuratedSeason, CuratedTitle, DownloadClient, DownloadState, DownloadedEpisode,
    Downloading, Failure, FoundItem, Fulfilling, Grabbed, Identity, Importing, ItemPart,
    Maintenance, MusicQuality, Pipeline, Protocol, QualityProfile, QualityReleases, Queue,
    QueueItem, Queues, Record, RegisteredClient, RegisteredFolder, ReleaseProbe, RootFolder,
    StuckItem, TitleRequest, TraceEvent,
};
use lemonfiber_ports::Client;

use super::{crosses_alike, curate, json, Upstream, SERVICE};

#[async_trait]
impl Client for Upstream {
    async fn identity(&self) -> Result<Identity, Failure> {
        Ok(Identity {
            name: "curator".to_owned(),
            version: "4.1".to_owned(),
        })
    }
    async fn register_download_client(&self, client: &DownloadClient) -> Result<(), Failure> {
        self.tell(format!("register {}", json(client)));
        Ok(())
    }
    async fn update_download_client(
        &self,
        id: &str,
        client: &DownloadClient,
    ) -> Result<(), Failure> {
        self.tell(format!("update {id} {}", json(client)));
        Ok(())
    }
    async fn set_client_field(
        &self,
        id: &str,
        field: &str,
        value: Option<&str>,
    ) -> Result<(), Failure> {
        self.tell(format!("set {id} {field} {value:?}"));
        Ok(())
    }
    async fn test_download_clients(&self) -> Result<Vec<ClientProbe>, Failure> {
        Ok(Vec::new())
    }
    async fn register_root_folder(&self, folder: &RootFolder) -> Result<(), Failure> {
        self.tell(format!("folder {folder:?}"));
        Ok(())
    }
    async fn root_folders(&self) -> Result<Vec<RegisteredFolder>, Failure> {
        Ok(Vec::new())
    }
    async fn download_clients(&self) -> Result<Vec<RegisteredClient>, Failure> {
        Ok(Vec::new())
    }
    async fn quality_profiles(&self) -> Result<Vec<QualityProfile>, Failure> {
        Err(Failure::Unauthorised {
            service: SERVICE.to_owned(),
        })
    }
}

#[async_trait]
impl Maintenance for Upstream {
    async fn search_upgrades(&self, kind: Kind) -> Result<(), Failure> {
        self.tell(format!("upgrades {kind:?}"));
        Ok(())
    }
    async fn wanting(&self) -> Result<bool, Failure> {
        Ok(true)
    }
}

#[async_trait]
impl Importing for Upstream {
    async fn hardlinks(&self) -> Result<bool, Failure> {
        Ok(true)
    }
    async fn set_hardlinks(&self, hardlink: bool) -> Result<(), Failure> {
        self.tell(format!("hardlinks {hardlink}"));
        Ok(())
    }
}

#[async_trait]
impl Carrying for Upstream {
    async fn records(&self, sort: Record) -> Result<Vec<Carried>, Failure> {
        Ok(vec![Carried {
            name: format!("{sort:?}"),
            profile: None,
            folder: Some("/films".to_owned()),
            rest: "{}".to_owned(),
        }])
    }
    async fn carry(&self, sort: Record, item: &Carried) -> Result<(), Failure> {
        self.tell(format!("carry {sort:?} {item:?}"));
        Ok(())
    }
}

#[async_trait]
impl Catalogue for Upstream {
    async fn lookup(&self, kind: Kind, term: &str) -> Result<Vec<CatalogueEntry>, Failure> {
        Ok(vec![CatalogueEntry {
            title: format!("{term} ({kind:?})"),
            year: Some(1999),
            reference: 603,
            held_as: None,
        }])
    }
    async fn add_plan(&self, kind: Kind) -> Result<AddPlan, Failure> {
        Ok(AddPlan {
            root_folder: format!("/{}", kind.noun()),
            quality_profile: 4,
        })
    }
    async fn add(
        &self,
        kind: Kind,
        entry: &CatalogueEntry,
        plan: &AddPlan,
    ) -> Result<Added, Failure> {
        self.tell(format!("add {kind:?} {entry:?} {plan:?}"));
        Ok(Added {
            id: entry.reference,
            title: entry.title.clone(),
        })
    }
    async fn indexer_count(&self) -> Result<usize, Failure> {
        Ok(3)
    }
}

#[async_trait]
impl Queues for Upstream {
    async fn queue(&self) -> Result<Queue, Failure> {
        Ok(Queue::default())
    }
}

#[async_trait]
impl QualityReleases for Upstream {
    async fn probe_releases(&self, kind: Kind) -> Result<ReleaseProbe, Failure> {
        Ok(match kind {
            Kind::Tv => ReleaseProbe::Matching,
            Kind::Movies => ReleaseProbe::NoneFound,
        })
    }
}

#[async_trait]
impl MusicQuality for Upstream {
    async fn apply_music_format(&self, format: Format) -> Result<(), Failure> {
        self.tell(format!("format {format:?}"));
        Ok(())
    }
}

#[async_trait]
impl Pipeline for Upstream {
    async fn library(&self, kind: Kind) -> Result<Vec<FoundItem>, Failure> {
        self.tell(format!("library {kind:?}"));
        Ok(Vec::new())
    }
    async fn find_items(&self, kind: Kind, term: &str) -> Result<Vec<FoundItem>, Failure> {
        self.tell(format!("find {kind:?} {term}"));
        Ok(Vec::new())
    }
    async fn item_history(&self, kind: Kind, id: i64) -> Result<Vec<TraceEvent>, Failure> {
        self.tell(format!("history {kind:?} {id}"));
        Ok(Vec::new())
    }
    async fn item_queue(&self, kind: Kind, id: i64) -> Result<Vec<QueueItem>, Failure> {
        self.tell(format!("queued {kind:?} {id}"));
        Ok(Vec::new())
    }
    async fn item_parts(
        &self,
        kind: Kind,
        id: i64,
        season: Option<u32>,
    ) -> Result<Vec<ItemPart>, Failure> {
        self.tell(format!("parts {kind:?} {id} {season:?}"));
        Ok(Vec::new())
    }
    async fn stuck_items(&self, kind: Kind) -> Result<Vec<StuckItem>, Failure> {
        self.tell(format!("stuck {kind:?}"));
        if kind == Kind::Movies {
            return Err(Failure::Unavailable {
                service: SERVICE.to_owned(),
            });
        }
        Ok(vec![StuckItem {
            title: "The Office".to_owned(),
            stage: Stage::Downloading,
            queued: Some(41),
            title_id: Some(9),
            release: Some(FINGERPRINT.to_owned()),
        }])
    }
}

/// A series held as `id`, its first season monitored and filed whole.
fn held(reference: i64, id: Option<i64>) -> CuratedTitle {
    CuratedTitle {
        reference,
        id,
        title: "The Office".to_owned(),
        year: Some(2005),
        monitored: true,
        files: 6,
        seasons: vec![CuratedSeason {
            number: 1,
            monitored: true,
            files: 6,
            episodes: 6,
        }],
    }
}

#[async_trait]
impl Fulfilling for Upstream {
    async fn catalogued(
        &self,
        kind: Kind,
        reference: i64,
    ) -> Result<Option<CuratedTitle>, Failure> {
        self.tell(format!("catalogued {kind:?} {reference}"));
        Ok((reference != 0).then(|| held(reference, None)))
    }
    async fn titles(&self, kind: Kind) -> Result<Vec<CuratedTitle>, Failure> {
        self.tell(format!("titles {kind:?}"));
        Ok(vec![held(73244, Some(9))])
    }
    async fn held_title(&self, kind: Kind, id: i64) -> Result<Option<CuratedTitle>, Failure> {
        self.tell(format!("held {kind:?} {id}"));
        Ok(None)
    }
    async fn request(&self, kind: Kind, request: &TitleRequest) -> Result<CuratedTitle, Failure> {
        self.tell(format!("request {kind:?} {}", json(request)));
        Ok(held(request.reference, Some(9)))
    }
    async fn search(&self, kind: Kind, id: i64) -> Result<(), Failure> {
        self.tell(format!("search {kind:?} {id}"));
        Ok(())
    }
    async fn remove(&self, kind: Kind, id: i64) -> Result<(), Failure> {
        self.tell(format!("remove {kind:?} {id}"));
        Err(Failure::Refused {
            service: SERVICE.to_owned(),
            detail: "it holds no title 0".to_owned(),
        })
    }
    async fn unqueue(&self, kind: Kind, id: i64, blocklist: bool) -> Result<String, Failure> {
        self.tell(format!("unqueue {kind:?} {id} {blocklist}"));
        if id == 0 {
            return Err(Failure::Refused {
                service: SERVICE.to_owned(),
                detail: "it holds no queue entry 0".to_owned(),
            });
        }
        Ok(FINGERPRINT.to_owned())
    }
    async fn grab(&self, kind: Kind, id: i64, release: &str) -> Result<Grabbed, Failure> {
        self.tell(format!("grab {kind:?} {id} {release}"));
        Ok(if release == FINGERPRINT {
            Grabbed::Grabbed
        } else {
            Grabbed::Gone
        })
    }
    async fn downloading(&self, kind: Kind) -> Result<Vec<Downloading>, Failure> {
        self.tell(format!("downloading {kind:?}"));
        Ok(vec![Downloading {
            title_id: 9,
            release: "The.Office.S01E02".to_owned(),
            bytes: 1000,
            left: 250,
            expected: Some(1_800_000_000),
            state: DownloadState::Downloading,
            download_id: Some("SABnzbd_nzo_1".to_owned()),
            episode: Some(DownloadedEpisode {
                season: 1,
                number: 2,
                title: "Diversity Day".to_owned(),
            }),
        }])
    }
}

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
        format!("{:?}", curator.probe_releases(Kind::Tv).await),
        format!("{:?}", curator.probe_releases(Kind::Movies).await),
        format!("{:?}", curator.apply_music_format(Format::Lossless).await),
        format!("{:?}", curator.library(Kind::Movies).await),
        format!("{:?}", curator.find_items(Kind::Tv, "office").await),
        format!("{:?}", curator.item_history(Kind::Tv, 12).await),
        format!("{:?}", curator.item_queue(Kind::Movies, 13).await),
        format!("{:?}", curator.item_parts(Kind::Tv, 14, Some(2)).await),
        format!("{:?}", curator.item_parts(Kind::Tv, 14, None).await),
        format!("{:?}", curator.stuck_items(Kind::Movies).await),
        format!("{:?}", curator.catalogued(Kind::Tv, 73244).await),
        format!("{:?}", curator.catalogued(Kind::Movies, 0).await),
        format!("{:?}", curator.titles(Kind::Tv).await),
        format!("{:?}", curator.held_title(Kind::Movies, 3).await),
        format!("{:?}", curator.request(Kind::Tv, &request).await),
        format!("{:?}", curator.search(Kind::Tv, 9).await),
        format!("{:?}", curator.remove(Kind::Movies, 0).await),
        format!("{:?}", curator.downloading(Kind::Tv).await),
        format!("{:?}", curator.stuck_items(Kind::Tv).await),
        format!("{:?}", curator.unqueue(Kind::Tv, 41, true).await),
        format!("{:?}", curator.unqueue(Kind::Movies, 0, false).await),
        format!("{:?}", curator.grab(Kind::Tv, 9, FINGERPRINT).await),
        format!("{:?}", curator.grab(Kind::Movies, 7, "elsewhere").await),
    ]
}

/// A release's fingerprint, as a curator answers it.
const FINGERPRINT: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

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
