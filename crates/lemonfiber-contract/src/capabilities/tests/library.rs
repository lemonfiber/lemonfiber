//! `library.curate` across its contract: one script run against a curator in process and
//! against the same curator through the contract, which must answer alike and be told
//! alike: a failure is named after the service the core called, as the core names it.

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_ports::media::{Format, Kind};
use lemonfiber_ports::service::{
    AddPlan, Added, Carried, Carrying, Catalogue, CatalogueEntry, Category, ClientProbe,
    Credential, DownloadClient, Failure, FoundItem, Identity, Importing, ItemPart, Maintenance,
    MusicQuality, Pipeline, Protocol, QualityProfile, QualityReleases, Queue, QueueItem, Queues,
    Record, RegisteredClient, RegisteredFolder, ReleaseProbe, RootFolder, StuckItem, TraceEvent,
};
use lemonfiber_ports::Client;

use super::{contracted, curate, Served, Upstream, SERVICE};

#[async_trait]
impl Client for Upstream {
    async fn identity(&self) -> Result<Identity, Failure> {
        Ok(Identity {
            name: "curator".to_owned(),
            version: "4.1".to_owned(),
        })
    }
    async fn register_download_client(&self, client: &DownloadClient) -> Result<(), Failure> {
        self.tell(format!("register {client:?}"));
        Ok(())
    }
    async fn update_download_client(
        &self,
        id: &str,
        client: &DownloadClient,
    ) -> Result<(), Failure> {
        self.tell(format!("update {id} {client:?}"));
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
        Err(Failure::Unavailable {
            service: SERVICE.to_owned(),
        })
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
        + Pipeline,
{
    let client = DownloadClient {
        name: "fetcher".to_owned(),
        host: "fetcher".to_owned(),
        port: 6789,
        protocol: Protocol("usenet".to_owned()),
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
    ]
}

#[tokio::test]
async fn a_curator_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    let in_process = Upstream::default();
    let local = script(&in_process).await;
    let served = Served::default();
    let reached = Arc::clone(&served.upstream);
    let crossed = script(&curate::Adapter(contracted(served))).await;
    assert_eq!(crossed, local);
    assert_eq!(reached.told(), in_process.told());
    assert_eq!(in_process.told().len(), 17);
}
