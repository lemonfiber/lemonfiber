//! `library.curate`: what a curator is asked, the service that files one kind of media
//! and fetches what is wanted of it.
//!
//! The ports a curator answers, grouped as one contract: where it downloads to and files
//! under, what it holds and is looking for, what it would grab at the quality asked of it,
//! and how a household's request reaches it. Every kind of media is named by what it is,
//! never by the service that files it.

use lemonfiber_ports::media::{Format, Kind};
use lemonfiber_ports::service::{
    AddPlan, Added, Carried, Carrying, Catalogue, CatalogueEntry, ClientProbe, DownloadClient,
    FoundItem, Identity, Importing, ItemPart, Maintenance, MusicQuality, Pipeline, QualityProfile,
    QualityReleases, Queue, QueueItem, Queues, Record, RegisteredClient, RegisteredFolder,
    ReleaseProbe, RootFolder, StuckItem, TraceEvent,
};
use lemonfiber_ports::Client;

crate::contract! {
    /// `library.curate`: a curator.
    pub mod curate = "library.curate" @ 1 {
        impl Client {
            /// Who it is and which release it runs.
            fn identity() -> Identity;
            /// Register a download client.
            fn register_download_client(refer client: &DownloadClient as DownloadClient) -> ();
            /// Change a download client it already holds.
            fn update_download_client(
                str id: &str,
                refer client: &DownloadClient as DownloadClient,
            ) -> ();
            /// Set one field of a download client it holds.
            fn set_client_field(str id: &str, str field: &str, opt_str value: Option<&str>) -> ();
            /// Test every download client it holds.
            fn test_download_clients() -> Vec<ClientProbe>;
            /// Register a folder it files into.
            fn register_root_folder(refer folder: &RootFolder as RootFolder) -> ();
            /// The folders it files into.
            fn root_folders() -> Vec<RegisteredFolder>;
            /// The download clients it holds.
            fn download_clients() -> Vec<RegisteredClient>;
            /// Its quality profiles.
            fn quality_profiles() -> Vec<QualityProfile>;
        }
        impl Maintenance {
            /// Re-search what it holds of one kind for a better release.
            fn search_upgrades(value kind: Kind) -> ();
        }
        impl Importing {
            /// Whether it imports by hard link.
            fn hardlinks() -> bool;
            /// Import by hard link, or not.
            fn set_hardlinks(value hardlink: bool) -> ();
        }
        impl Carrying {
            /// The records of one sort it holds, for carrying to another curator.
            fn records(value sort: Record) -> Vec<Carried>;
            /// Take one record carried from another curator.
            fn carry(value sort: Record, refer item: &Carried as Carried) -> ();
        }
        impl Catalogue {
            /// What the catalogue holds of one kind for a term.
            fn lookup(value kind: Kind, str term: &str) -> Vec<CatalogueEntry>;
            /// How it would add something of one kind.
            fn add_plan(value kind: Kind) -> AddPlan;
            /// Add one entry, as planned.
            fn add(
                value kind: Kind,
                refer entry: &CatalogueEntry as CatalogueEntry,
                refer plan: &AddPlan as AddPlan,
            ) -> Added;
            /// How many indexers it searches.
            fn indexer_count() -> usize;
        }
        impl Queues {
            /// What is queued.
            fn queue() -> Queue;
        }
        impl QualityReleases {
            /// What it would grab for one wanted item of one kind.
            fn probe_releases(value kind: Kind) -> ReleaseProbe;
        }
        impl MusicQuality {
            /// Apply a music format.
            fn apply_music_format(value format: Format) -> ();
        }
        impl Pipeline {
            /// Everything it holds of one kind.
            fn library(value kind: Kind) -> Vec<FoundItem>;
            /// What it holds of one kind for a term.
            fn find_items(value kind: Kind, str term: &str) -> Vec<FoundItem>;
            /// What happened to one item.
            fn item_history(value kind: Kind, value id: i64) -> Vec<TraceEvent>;
            /// What of one item is queued.
            fn item_queue(value kind: Kind, value id: i64) -> Vec<QueueItem>;
            /// The parts of one item, in one season where it has seasons.
            fn item_parts(value kind: Kind, value id: i64, value season: Option<u32>) -> Vec<ItemPart>;
            /// What of one kind is stuck.
            fn stuck_items(value kind: Kind) -> Vec<StuckItem>;
        }
    }
}
