//! The upstream as a library curator.

use async_trait::async_trait;
use lemonfiber_ports::media::{Format, Kind, TitleKind};
use lemonfiber_ports::service::stage::Stage;
use lemonfiber_ports::service::{
    AddPlan, Added, Carried, Carrying, Catalogue, CatalogueEntry, ClientProbe, CuratedSeason,
    CuratedTitle, DownloadClient, DownloadState, DownloadedEpisode, Downloading, Failure,
    FoundItem, Fulfilling, Grabbed, Identity, Importing, ItemPart, Maintenance, MusicQuality,
    Pipeline, QualityProfile, QualityReleases, Queue, QueueItem, Queues, Record, RegisteredClient,
    RegisteredFolder, ReleaseProbe, RootFolder, StuckItem, TitleRequest, TraceEvent,
};
use lemonfiber_ports::Client;

use super::{json, Upstream, SERVICE};

/// A release's fingerprint, as a curator answers it.
pub const FINGERPRINT: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

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
    async fn probe_releases(&self, kind: TitleKind) -> Result<ReleaseProbe, Failure> {
        Ok(match kind {
            TitleKind::Tv => ReleaseProbe::Matching,
            TitleKind::Movies => ReleaseProbe::NoneFound,
            TitleKind::Music => ReleaseProbe::NoneMatch,
            TitleKind::Books => ReleaseProbe::NothingWanted,
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
    async fn library(&self, kind: TitleKind) -> Result<Vec<FoundItem>, Failure> {
        self.tell(format!("library {kind:?}"));
        Ok(Vec::new())
    }
    async fn find_items(&self, kind: TitleKind, term: &str) -> Result<Vec<FoundItem>, Failure> {
        self.tell(format!("find {kind:?} {term}"));
        Ok(Vec::new())
    }
    async fn item_history(&self, kind: TitleKind, id: i64) -> Result<Vec<TraceEvent>, Failure> {
        self.tell(format!("history {kind:?} {id}"));
        Ok(Vec::new())
    }
    async fn item_queue(&self, kind: TitleKind, id: i64) -> Result<Vec<QueueItem>, Failure> {
        self.tell(format!("queued {kind:?} {id}"));
        Ok(Vec::new())
    }
    async fn item_parts(
        &self,
        kind: TitleKind,
        id: i64,
        season: Option<u32>,
    ) -> Result<Vec<ItemPart>, Failure> {
        self.tell(format!("parts {kind:?} {id} {season:?}"));
        Ok(Vec::new())
    }
    async fn stuck_items(&self, kind: TitleKind) -> Result<Vec<StuckItem>, Failure> {
        self.tell(format!("stuck {kind:?}"));
        if kind == TitleKind::Movies {
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
    async fn titles(&self, kind: TitleKind) -> Result<Vec<CuratedTitle>, Failure> {
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
    async fn search(&self, kind: TitleKind, id: i64) -> Result<(), Failure> {
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
    async fn unqueue(&self, kind: TitleKind, id: i64, blocklist: bool) -> Result<String, Failure> {
        self.tell(format!("unqueue {kind:?} {id} {blocklist}"));
        if id == 0 {
            return Err(Failure::Refused {
                service: SERVICE.to_owned(),
                detail: "it holds no queue entry 0".to_owned(),
            });
        }
        Ok(FINGERPRINT.to_owned())
    }
    async fn grab(&self, kind: TitleKind, id: i64, release: &str) -> Result<Grabbed, Failure> {
        self.tell(format!("grab {kind:?} {id} {release}"));
        Ok(if release == FINGERPRINT {
            Grabbed::Grabbed
        } else {
            Grabbed::Gone
        })
    }
    async fn downloading(&self, kind: TitleKind) -> Result<Vec<Downloading>, Failure> {
        self.tell(format!("downloading {kind:?}"));
        Ok(vec![Downloading {
            title_id: 9,
            release: "The.Office.S01E02".to_owned(),
            bytes: 1000,
            left: 250,
            expected: Some(1_800_000_000),
            state: DownloadState::Downloading,
            download_id: Some("SABnzbd_nzo_1".to_owned()),
            queued: Some(4),
            episode: Some(DownloadedEpisode {
                season: 1,
                number: 2,
                title: "Diversity Day".to_owned(),
            }),
        }])
    }
}
