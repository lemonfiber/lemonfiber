//! `library.curate`: what a curator is asked, the service that files one kind of media
//! and fetches what is wanted of it.
//!
//! The ports a curator answers, grouped as one contract: where it downloads to and files
//! under, what it holds and is looking for, what it would grab at the quality asked of it,
//! and how a household's request reaches it. Every kind of media is named by what it is,
//! never by the service that files it.

use lemonfiber_ports::media::{Format, Kind, TitleKind};
use lemonfiber_ports::service::{
    AddPlan, Added, Carried, Carrying, Catalogue, CatalogueEntry, Category, ClientProbe,
    Credential, CuratedTitle, DownloadClient, Downloading, FoundItem, Fulfilling, Grabbed, Heard,
    Identity, Importing, ItemPart, Maintenance, MusicQuality, Pipeline, Protocol, QualityProfile,
    QualityReleases, Queue, QueueItem, Queues, Record, Recounting, RegisteredClient,
    RegisteredFolder, ReleaseProbe, RootFolder, StuckItem, TitleRequest, TraceEvent,
};
use lemonfiber_ports::serving::Said;
use lemonfiber_ports::Client;
use serde_json::Value;

use super::{FOLDER, NAMED, PROFILE, TOLD_AT, TOLD_EVERY, TOLD_PINNED, TOLD_WITH};
use crate::conformance::Placeholder;

/// The host the conformance cases name a download client on.
pub const CLIENT_HOST: &str = "conformance-download";

/// The port the conformance cases name a download client on: `SABnzbd`'s own.
const CLIENT_PORT: u16 = 8080;

/// The version the client the cases register answers it is.
const CLIENT_VERSION: &str = "4.5.1";

/// The folder the client the cases register says it finishes downloads in.
const CLIENT_FINISHED: &str = "/downloads/complete";

/// Where the client the cases register is asked.
const CLIENT_API: &str = "/api";

/// The query parameter that names what the client the cases register is asked.
const ASKED_BY: &str = "mode";

/// The protocol the conformance cases register their download client in: the native API
/// of the download filler the bundle ships, as the core names the clients it registers.
const CLIENT_PROTOCOL: &str = "sabnzbd";

/// Every operation that takes a `Kind`, each asked once for each kind of video.
const BY_KIND: [&str; 8] = [
    "search_upgrades",
    "lookup",
    "add_plan",
    "add",
    "catalogued",
    "held_title",
    "request",
    "remove",
];

/// Every operation that takes a `TitleKind`, each asked once for each kind.
const BY_TITLE_KIND: [&str; 12] = [
    "probe_releases",
    "library",
    "find_items",
    "item_history",
    "item_queue",
    "item_parts",
    "stuck_items",
    "titles",
    "search",
    "downloading",
    "unqueue",
    "grab",
];

/// The minor at which every operation of [`BY_TITLE_KIND`] takes music and books.
const ALBUMS_AND_BOOKS: u32 = 3;

/// The operations the request gate asks a curator under its request key, and no other.
pub const REQUESTED: [&str; 10] = [
    "catalogued",
    "titles",
    "held_title",
    "request",
    "search",
    "remove",
    "downloading",
    "quality_profiles",
    "root_folders",
    "lookup",
];

/// Every operation that takes a `Record` sort, each asked once for each media type's.
const BY_RECORD: [&str; 2] = ["records", "carry"];

/// The one operation that names a root folder's media type.
const BY_FOLDER: &str = "register_root_folder";

/// The field the conformance cases set on a download client.
const FIELD: &str = "category";

/// An item no curator holds.
const NO_ITEM: i64 = 0;

/// The fingerprint a case asks to grab, of a release no curator holds.
const FINGERPRINT: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// What answers as the download client the conformance cases register while a curator is
/// recorded: `SABnzbd`'s answers to the queries a curator tests a client with before it saves
/// one, filing under the category the cases name and each of `media_types`, and nothing
/// more.
#[must_use]
pub fn client_answering(media_types: &[String]) -> lemonfiber_ports::serving::StandIn {
    let named: Vec<&str> = ["*", NAMED]
        .into_iter()
        .chain(media_types.iter().map(String::as_str))
        .collect();
    let categories: Vec<Value> = named
        .iter()
        .map(|name| serde_json::json!({ "name": name, "dir": "" }))
        .collect();
    let answers = [
        ("version", serde_json::json!({ "version": CLIENT_VERSION })),
        (
            "get_config",
            serde_json::json!({ "config": {
                "misc": {
                    "complete_dir": CLIENT_FINISHED,
                    "pre_check": false,
                    "enable_tv_sorting": false,
                    "enable_movie_sorting": false,
                    "enable_date_sorting": false,
                },
                "categories": categories,
                "sorters": [],
            } }),
        ),
        (
            "queue",
            serde_json::json!({ "queue": { "slots": [], "paused": false } }),
        ),
        ("history", serde_json::json!({ "history": { "slots": [] } })),
        ("get_cats", serde_json::json!({ "categories": named })),
    ];
    lemonfiber_ports::serving::StandIn {
        port: CLIENT_PORT,
        headers: Vec::new(),
        answers: answers
            .into_iter()
            .map(|(mode, said)| Said {
                query: Some((ASKED_BY.to_owned(), mode.to_owned())),
                ..Said::json("GET", CLIENT_API, said.to_string())
            })
            .collect(),
    }
}

/// The download client the conformance cases register.
fn client() -> DownloadClient {
    DownloadClient {
        name: NAMED.to_owned(),
        host: CLIENT_HOST.to_owned(),
        port: CLIENT_PORT,
        protocol: Protocol(CLIENT_PROTOCOL.to_owned()),
        credential: Credential::ApiKey(Placeholder::Minted.written()),
        category: Category {
            field: FIELD.to_owned(),
            value: NAMED.to_owned(),
        },
    }
}

/// The download client this contract's `download_clients` case answered first.
fn held() -> String {
    Placeholder::answer(
        curate::CAPABILITY,
        curate::MAJOR,
        "download_clients",
        "/0/id",
    )
    .written()
}

crate::contract! {
    /// `library.curate`: a curator.
    pub mod curate = "library.curate" @ 1 holds [
        crate::Held::carried("followed-series", "Followed series"),
        crate::Held::carried("followed-films", "Followed films"),
        crate::Held::carried("followed-artists", "Followed artists"),
        crate::Held::carried("followed-authors", "Followed authors"),
        crate::Held::carried("indexers", "Indexers"),
        crate::Held::kept("quality-profiles", "Quality profiles edited by hand"),
        crate::Held::kept("download-history", "Download history"),
    ] {
        impl Client {
            /// Who it is and which release it runs.
            fn identity() -> Identity where asks = Asked {};
            /// Register a download client.
            fn register_download_client(refer client: &DownloadClient as DownloadClient) -> () where asks = Asked { client: client() };
            /// The download clients it holds.
            fn download_clients() -> Vec<RegisteredClient> where asks = Asked {};
            /// Change a download client it already holds.
            fn update_download_client(
                str id: &str,
                refer client: &DownloadClient as DownloadClient,
            ) -> () where asks = Asked { id: held(), client: client() };
            /// Set one field of a download client it holds.
            fn set_client_field(str id: &str, str field: &str, opt_str value: Option<&str>) -> () where asks = Asked { id: held(), field: FIELD.to_owned(), value: Some(NAMED.to_owned()) };
            /// Test every download client it holds.
            fn test_download_clients() -> Vec<ClientProbe> where asks = Asked {};
            /// Register a folder it files into.
            fn register_root_folder(refer folder: &RootFolder as RootFolder) -> () where asks = Asked {
                folder: RootFolder {
                    path: FOLDER.to_owned(),
                    media_type: Kind::Tv.media_type().to_owned(),
                },
            };
            /// The folders it files into.
            fn root_folders() -> Vec<RegisteredFolder> where asks = Asked {};
            /// Its quality profiles.
            fn quality_profiles() -> Vec<QualityProfile> where asks = Asked {};
        }
        impl Maintenance {
            /// Re-search what it holds of one kind for a better release.
            fn search_upgrades(value kind: Kind) -> () where asks = Asked { kind: Kind::Tv };
            /// Whether anything is on its wanted list, read without searching.
            fn wanting() -> bool where asks = Asked {};
        }
        impl Importing {
            /// Whether it imports by hard link.
            fn hardlinks() -> bool where asks = Asked {};
            /// Import by hard link, or not.
            fn set_hardlinks(value hardlink: bool) -> () where asks = Asked { hardlink: true };
        }
        impl Carrying {
            /// The records of one sort it holds, for carrying to another curator.
            fn records(value sort: Record) -> Vec<Carried> where asks = Asked { sort: Record::Series };
            /// Take one record carried from another curator.
            fn carry(value sort: Record, refer item: &Carried as Carried) -> () where asks = Asked {
                sort: Record::Series,
                item: Carried {
                    name: NAMED.to_owned(),
                    profile: None,
                    folder: None,
                    rest: String::new(),
                },
            };
        }
        impl Catalogue {
            /// What the catalogue holds of one kind for a term.
            fn lookup(value kind: Kind, str term: &str) -> Vec<CatalogueEntry> where asks = Asked { kind: Kind::Tv, term: NAMED.to_owned() };
            /// How it would add something of one kind.
            fn add_plan(value kind: Kind) -> AddPlan where asks = Asked { kind: Kind::Tv };
            /// Add one entry, as planned.
            fn add(
                value kind: Kind,
                refer entry: &CatalogueEntry as CatalogueEntry,
                refer plan: &AddPlan as AddPlan,
            ) -> Added where asks = Asked {
                kind: Kind::Tv,
                entry: CatalogueEntry {
                    title: NAMED.to_owned(),
                    year: None,
                    reference: NO_ITEM,
                    held_as: None,
                },
                plan: AddPlan {
                    root_folder: FOLDER.to_owned(),
                    quality_profile: i64::from(PROFILE),
                },
            };
            /// How many indexers it searches.
            fn indexer_count() -> usize where asks = Asked {};
        }
        impl Queues {
            /// What is queued.
            fn queue() -> Queue where asks = Asked {};
        }
        impl QualityReleases {
            /// What it would grab for one wanted item of one kind.
            fn probe_releases(value kind: TitleKind) -> ReleaseProbe where asks = Asked { kind: TitleKind::Tv };
        }
        impl MusicQuality {
            /// Apply a music format.
            fn apply_music_format(value format: Format) -> () where asks = Asked { format: Format::Lossless };
        }
        impl Pipeline {
            /// Everything it holds of one kind.
            fn library(value kind: TitleKind) -> Vec<FoundItem> where asks = Asked { kind: TitleKind::Tv };
            /// What it holds of one kind for a term.
            fn find_items(value kind: TitleKind, str term: &str) -> Vec<FoundItem> where asks = Asked { kind: TitleKind::Tv, term: NAMED.to_owned() };
            /// What happened to one item.
            fn item_history(value kind: TitleKind, value id: i64) -> Vec<TraceEvent> where asks = Asked { kind: TitleKind::Tv, id: NO_ITEM };
            /// What of one item is queued.
            fn item_queue(value kind: TitleKind, value id: i64) -> Vec<QueueItem> where asks = Asked { kind: TitleKind::Tv, id: NO_ITEM };
            /// The parts of one item, in one season where it has seasons.
            fn item_parts(value kind: TitleKind, value id: i64, value season: Option<u32>) -> Vec<ItemPart> where asks = Asked { kind: TitleKind::Tv, id: NO_ITEM, season: None };
            /// What of one kind is stuck.
            fn stuck_items(value kind: TitleKind) -> Vec<StuckItem> where asks = Asked { kind: TitleKind::Tv };
        }
        impl Fulfilling {
            /// The title of one kind its catalogue holds by a reference, or none.
            fn catalogued(value kind: Kind, value reference: i64) -> Option<CuratedTitle> where since = 1, asks = Asked { kind: Kind::Tv, reference: NO_ITEM };
            /// Every title of one kind it holds.
            fn titles(value kind: TitleKind) -> Vec<CuratedTitle> where since = 1, asks = Asked { kind: TitleKind::Tv };
            /// One title of one kind it holds, or none.
            fn held_title(value kind: Kind, value id: i64) -> Option<CuratedTitle> where since = 1, asks = Asked { kind: Kind::Tv, id: NO_ITEM };
            /// Take a household's request for one title.
            fn request(value kind: Kind, refer request: &TitleRequest as TitleRequest) -> CuratedTitle where since = 1, asks = Asked {
                kind: Kind::Tv,
                request: TitleRequest {
                    reference: NO_ITEM,
                    root_folder: FOLDER.to_owned(),
                    quality_profile: i64::from(PROFILE),
                    seasons: vec![1],
                    anime: false,
                    later_seasons: false,
                    search: false,
                },
            };
            /// Search for what it wants of one title it holds.
            fn search(value kind: TitleKind, value id: i64) -> () where since = 1, asks = Asked { kind: TitleKind::Tv, id: NO_ITEM };
            /// Remove one title it holds, with its files.
            fn remove(value kind: Kind, value id: i64) -> () where since = 1, asks = Asked { kind: Kind::Tv, id: NO_ITEM };
            /// What it is downloading of one kind.
            fn downloading(value kind: TitleKind) -> Vec<Downloading> where since = 1, asks = Asked { kind: TitleKind::Tv };
            /// Drop one queue entry from its download client, blocklisting its release where asked.
            fn unqueue(value kind: TitleKind, value id: i64, value blocklist: bool) -> String where since = 2, asks = Asked { kind: TitleKind::Tv, id: NO_ITEM, blocklist: true };
            /// Grab a release again by its fingerprint, lifting its blocklist entry.
            fn grab(value kind: TitleKind, value id: i64, str release: &str) -> Grabbed where since = 2, asks = Asked { kind: TitleKind::Tv, id: NO_ITEM, release: FINGERPRINT.to_owned() };
        }
        impl Recounting {
            /// Send each happening to the core's address, pinning the certificate it presents
            /// there, with the key the core minted for it, polling every so many seconds where
            /// it polls; answers how it hears, or nothing where it stopped.
            fn events(opt_str address: Option<&str>, opt_str pin: Option<&str>, opt_str key: Option<&str>, value every: Option<u32>) -> Option<Heard> where since = 3, asks = Asked { address: Some(TOLD_AT.to_owned()), pin: Some(TOLD_PINNED.to_owned()), key: Some(TOLD_WITH.to_owned()), every: Some(TOLD_EVERY) };
        }
    }
}

/// `library.curate`, every operation bound to a kind of media asked once for each media
/// type it takes.
#[must_use]
pub fn capability() -> crate::Capability {
    let mut curated = curate::capability();
    for operation in &mut curated.operations {
        operation.filed = filed(operation.name, &operation.asks);
    }
    curated
}

/// What the case of `operation`, which otherwise posts `asks`, posts for each media type
/// it is bound to; nothing where it is bound to none.
fn filed(operation: &str, asks: &Value) -> Vec<crate::MediaCase> {
    let each = |pointer: &str, by: Vec<(&'static str, Value, u32)>| {
        by.into_iter()
            .map(|(media_type, value, since)| {
                let mut asked = asks.clone();
                if let Some(at) = asked.pointer_mut(pointer) {
                    *at = value;
                }
                crate::MediaCase {
                    media_type,
                    asks: asked,
                    since,
                }
            })
            .collect()
    };
    if BY_KIND.contains(&operation) {
        return each(
            "/kind",
            Kind::ALL
                .iter()
                .map(|kind| (kind.media_type(), wire(kind), 0))
                .collect(),
        );
    }
    if BY_TITLE_KIND.contains(&operation) {
        return each(
            "/kind",
            TitleKind::ALL
                .iter()
                .map(|kind| {
                    let since = if kind.video().is_some() {
                        0
                    } else {
                        ALBUMS_AND_BOOKS
                    };
                    (kind.media_type(), wire(kind), since)
                })
                .collect(),
        );
    }
    if BY_RECORD.contains(&operation) {
        return each(
            "/sort",
            Record::FOLLOWED
                .iter()
                .map(|(media_type, sort)| (*media_type, wire(sort), 0))
                .collect(),
        );
    }
    if operation == BY_FOLDER {
        return each(
            "/folder/media_type",
            TitleKind::ALL
                .into_iter()
                .map(|kind| {
                    let media_type = kind.media_type();
                    (media_type, Value::String(media_type.to_owned()), 0)
                })
                .collect(),
        );
    }
    Vec::new()
}

/// `value` as the wire writes it.
fn wire<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}
