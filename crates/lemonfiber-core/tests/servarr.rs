//! The Servarr client, driven through the HTTP port against a fake transport.
//!
//! The client turns a request into an API call and reads what the service
//! answered; the fake is that service, replying with exactly the status and body
//! a test wants — so every branch of the identity and registration paths is
//! exercised with nothing running. The client speaks an async trait built on
//! another, so it is driven from here rather than from an in-crate test, where it
//! would be compiled twice and its coverage counted from the wrong copy.

use std::sync::Arc;

use lemonfiber_core::ports::http::{Http, Request};
use lemonfiber_core::ports::service::{
    AddPlan, Added, CatalogueEntry, Category, ClientKind, Credential, DownloadClient,
};
use lemonfiber_core::recyclarr::Kind;
use lemonfiber_core::servarr::Servarr;
use lemonfiber_fixtures::http::{Answer, Fake};

/// A Sonarr client over the given fake — the v3 the media *arrs answer at.
fn sonarr(fake: &Arc<Fake>) -> Servarr {
    let http: Arc<dyn Http> = fake.clone();
    Servarr::new(http, "http://sonarr:8989", "the-key", "sonarr", 3)
}

/// A `SABnzbd` download client: a Usenet client authenticated by an API key.
fn sabnzbd() -> DownloadClient {
    DownloadClient {
        name: "SABnzbd".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        kind: ClientKind::Sabnzbd,
        credential: Credential::ApiKey("sab-key".to_owned()),
        category: Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        },
    }
}

// ---- Lidarr music-quality apply ----

/// A Lidarr client over the given router — the v1 Lidarr answers at.
fn lidarr(router: &Arc<Fake>) -> Servarr {
    let http: Arc<dyn Http> = router.clone();
    Servarr::new(http, "http://lidarr:8686", "the-key", "lidarr", 1)
}

/// A profile list a GET returns: a stray non-object (skipped), an object with no id
/// (rewritten but not addressable, so not sent), and a full profile that is updated —
/// carrying the 24-bit format in its items so a hi-res choice has something to score.
const PROFILES: &str = r#"[
    1,
    {"upgradeAllowed":false,"cutoff":1006,"items":[
        {"id":1005,"name":"High Quality Lossy","allowed":false,"items":[]},
        {"id":1006,"name":"Lossless","allowed":false,"items":[]}
    ]},
    {"id":2,"upgradeAllowed":false,"cutoff":1006,"cutoffFormatScore":0,
     "formatItems":[{"format":9,"name":"lemonfiber: 24-bit","score":0}],
     "items":[
        {"id":1005,"name":"High Quality Lossy","allowed":false,"items":[]},
        {"id":1006,"name":"Lossless","allowed":false,"items":[]}
    ]}
]"#;

/// A \*arr of the given kind over the fake, at the endpoint that kind answers on.
fn of_kind(fake: &Arc<Fake>, kind: Kind) -> Servarr {
    let http: Arc<dyn Http> = fake.clone();
    match kind {
        Kind::Sonarr => Servarr::new(http, "http://sonarr:8989", "the-key", "sonarr", 3),
        Kind::Radarr => Servarr::new(http, "http://radarr:7878", "the-key", "radarr", 3),
    }
}

/// Ask one kind to take something on, and hand back the request that went out.
async fn taking_on(kind: Kind) -> Option<Request> {
    let fake = Fake::always(Answer::reply(201, r#"{"id":9,"title":"Sintel"}"#));
    let entry = CatalogueEntry {
        title: "Sintel".to_owned(),
        year: Some(2010),
        reference: 45745,
        held_as: None,
    };
    let plan = AddPlan {
        root_folder: "/data/media".to_owned(),
        quality_profile: 4,
    };
    let took = of_kind(&fake, kind).add(kind, &entry, &plan).await;
    assert_eq!(
        took.ok(),
        Some(Added {
            id: 9,
            title: "Sintel".to_owned()
        }),
        "the service said what it took on and the client did not read it back"
    );
    fake.requests().into_iter().next()
}

mod answering;
mod quality;
mod reading;
mod writing;
