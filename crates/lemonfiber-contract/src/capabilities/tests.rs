//! Each capability's client against its own dispatcher, through a transport that is
//! nothing but the two halves: what the core asks is what an adapter is handed, and what
//! an adapter answers is what the core reads.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_ports::http::{Http, Request, Response, Unreachable};
use lemonfiber_ports::service::{
    Download, Failure, Fetching, Filed, Filing, Metering, Moved, Peering, Pulling, Rates, Seeded,
    Seeding, Throttled, Throttling, Transfers, UsenetAccount, UsenetAccounts, Wanted,
};

use super::archive::extract;
use super::dashboard::show;
use super::download::{torrent, usenet};
use super::identity::source;
use super::indexer::{pull, search};
use super::invite::decline;
use super::library::curate;
use super::media::serve;
use super::network::egress;
use super::owner::claim;
use super::proxy::front;
use super::quality::sync;
use super::request::{gate, intake};
use super::stream::guard;
use super::subtitles::fetch;
use crate::{wire, Contracted};

mod archive;
mod dashboard;
mod identity;
mod indexer;
mod invite;
mod library;
mod media;
mod network;
mod owner;
mod parties;
mod proxy;
mod quality;
mod request;
mod stream;
mod subtitles;

/// A download client that answers every question with something recognisable.
struct Client;

#[async_trait]
impl Transfers for Client {
    async fn transfers(&self) -> Result<Vec<Download>, Failure> {
        Ok(vec![Download {
            name: "a film".to_owned(),
            progress: 40,
            speed: Some(1),
            eta: Some(Duration::from_secs(60)),
            remaining: Some(9),
        }])
    }
}

#[async_trait]
impl Fetching for Client {
    async fn pulling(&self) -> Result<Pulling, Failure> {
        Ok(Pulling::Fetching)
    }
    async fn stop(&self) -> Result<Pulling, Failure> {
        Ok(Pulling::Stopped)
    }
    async fn resume(&self) -> Result<Pulling, Failure> {
        Err(Failure::Unavailable {
            service: "upstream".to_owned(),
        })
    }
}

#[async_trait]
impl Throttling for Client {
    async fn throttled(&self) -> Result<Throttled, Failure> {
        Ok(Throttled {
            rates: Rates::default(),
            uploads: true,
            hours: None,
        })
    }
    async fn restrain(&self, wanted: &Wanted) -> Result<Throttled, Failure> {
        Ok(Throttled {
            rates: wanted.active,
            uploads: false,
            hours: None,
        })
    }
    async fn moving(&self) -> Result<Rates, Failure> {
        Ok(Rates {
            down: Some(5),
            up: None,
        })
    }
}

#[async_trait]
impl Metering for Client {
    async fn moved(&self, month: &str) -> Result<Moved, Failure> {
        Ok(Moved {
            down: month.len() as u64,
            up: 0,
            since_start: false,
        })
    }
}

#[async_trait]
impl Seeding for Client {
    async fn seeding(&self) -> Result<Vec<Seeded>, Failure> {
        Ok(vec![Seeded {
            name: "a film".to_owned(),
            bytes: 1,
            ratio: 150,
        }])
    }

    async fn stop_seeding(&self, name: &str) -> Result<(), Failure> {
        if name == "a film" {
            Ok(())
        } else {
            Err(Failure::Refused {
                service: "upstream".to_owned(),
                detail: format!("nothing called {name} is held"),
            })
        }
    }
}

#[async_trait]
impl Peering for Client {
    async fn peer_port(&self) -> Result<Option<u16>, Failure> {
        Ok(Some(51413))
    }
    async fn set_peer_port(&self, port: Option<u16>) -> Result<(), Failure> {
        match port {
            Some(0) => Err(Failure::Refused {
                service: "upstream".to_owned(),
                detail: "no client takes peers on port 0".to_owned(),
            }),
            Some(_) | None => Ok(()),
        }
    }
}

#[async_trait]
impl Filing for Client {
    async fn filed(&self) -> Result<Vec<Filed>, Failure> {
        Ok(vec![Filed {
            category: "tv".to_owned(),
            folder: "/data/downloads/complete/tv".to_owned(),
        }])
    }
    async fn file_under(&self, folders: &[Filed]) -> Result<(), Failure> {
        match folders.iter().find(|one| one.folder.is_empty()) {
            Some(unfiled) => Err(Failure::Refused {
                service: "upstream".to_owned(),
                detail: format!("{} names no folder", unfiled.category),
            }),
            None => Ok(()),
        }
    }
}

#[async_trait]
impl UsenetAccounts for Client {
    async fn accounts(&self) -> Result<Vec<UsenetAccount>, Failure> {
        Ok(Vec::new())
    }
}

/// An upstream that writes down every argument it is handed and answers from them, so an
/// argument lost or altered on the way shows in what it was told and in what it said.
///
/// Each capability's tests implement that capability's ports for it.
#[derive(Default)]
pub(crate) struct Upstream {
    told: Mutex<Vec<String>>,
}

impl Upstream {
    fn tell(&self, what: String) {
        if let Ok(mut told) = self.told.lock() {
            told.push(what);
        }
    }

    pub(crate) fn told(&self) -> Vec<String> {
        self.told
            .lock()
            .map(|told| told.clone())
            .unwrap_or_default()
    }

    /// The operations it was told about, each by the word its entry starts with.
    fn operations(&self) -> Vec<String> {
        self.told()
            .iter()
            .map(|told| told.split(' ').next().unwrap_or_default().to_owned())
            .collect()
    }
}

/// One script run against an upstream in process and through the contract by `$adapter`:
/// both must answer alike, the upstream must be told alike, and it must be told exactly
/// the operations named.
macro_rules! crosses_alike {
    ($script:ident, $adapter:path, [$($told:literal),* $(,)?]) => {{
        let in_process = super::Upstream::default();
        let local = $script(&in_process).await;
        let served = super::Served::default();
        let reached = std::sync::Arc::clone(&served.upstream);
        let crossed = $script(&$adapter(super::contracted(served))).await;
        assert_eq!(crossed, local);
        assert_eq!(reached.told(), in_process.told());
        assert_eq!(in_process.operations(), [$($told),*]);
    }};
}
use crosses_alike;

/// A value as it crosses, credentials and all.
fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// A transport that hands every call to the dispatcher of the capability its path names.
#[derive(Default)]
struct Served {
    /// The upstream every call that is not a download client's reaches.
    upstream: Arc<Upstream>,
}

#[async_trait]
impl Http for Served {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        let mut segments = request.url.rsplit('/');
        let operation = segments.next().unwrap_or_default();
        let capability = segments.nth(1).unwrap_or_default();
        let body = request.body.as_deref().unwrap_or_default().as_bytes();
        let served = match capability {
            torrent::CAPABILITY => torrent::dispatch(&Client, operation, body).await,
            egress::CAPABILITY => egress::dispatch(&*self.upstream, operation, body).await,
            usenet::CAPABILITY => usenet::dispatch(&Client, operation, body).await,
            curate::CAPABILITY => curate::dispatch(&*self.upstream, operation, body).await,
            search::CAPABILITY => search::dispatch(&*self.upstream, operation, body).await,
            pull::CAPABILITY => pull::dispatch(&*self.upstream, operation, body).await,
            fetch::CAPABILITY => fetch::dispatch(&*self.upstream, operation, body).await,
            intake::CAPABILITY => intake::dispatch(&*self.upstream, operation, body).await,
            source::CAPABILITY => source::dispatch(&*self.upstream, operation, body).await,
            serve::CAPABILITY => serve::dispatch(&*self.upstream, operation, body).await,
            front::CAPABILITY => front::dispatch(&*self.upstream, operation, body).await,
            sync::CAPABILITY => sync::dispatch(&*self.upstream, operation, body).await,
            show::CAPABILITY => show::dispatch(&*self.upstream, operation, body).await,
            extract::CAPABILITY => extract::dispatch(&*self.upstream, operation, body).await,
            guard::CAPABILITY => guard::dispatch(&*self.upstream, operation, body).await,
            decline::CAPABILITY => decline::dispatch(&*self.upstream, operation, body).await,
            gate::CAPABILITY => gate::dispatch(&*self.upstream, operation, body).await,
            claim::CAPABILITY => claim::dispatch(&*self.upstream, operation, body).await,
            _ => Err(wire::Refusal::unknown_operation(operation)),
        };
        Ok(match served {
            Ok(wire::Answer::Json(body)) => Response {
                status: 200,
                headers: Vec::new(),
                body: String::from_utf8(body).unwrap_or_default(),
            },
            Ok(wire::Answer::Reset) => Response {
                status: wire::RESET,
                headers: Vec::new(),
                body: String::new(),
            },
            Err(refusal) => Response {
                status: refusal.kind.status(),
                headers: vec![("Content-Type".to_owned(), wire::PROBLEM.to_owned())],
                body: serde_json::to_string(&refusal).unwrap_or_default(),
            },
        })
    }
}

/// The service the core knows every adapter here as, and names its failures after.
const SERVICE: &str = "adapter";

/// The core's end of a contract, over a transport serving `served`.
fn contracted(served: Served) -> Contracted {
    Contracted::new(Arc::new(served), "http://adapter", SERVICE, "k")
}

#[tokio::test]
async fn a_torrent_client_answers_through_its_contract_as_it_answers_in_process() {
    let asked = torrent::Adapter(contracted(Served::default()));
    assert_eq!(asked.transfers().await.ok(), Client.transfers().await.ok());
    assert_eq!(asked.pulling().await.ok(), Some(Pulling::Fetching));
    assert_eq!(asked.stop().await.ok(), Some(Pulling::Stopped));
    assert!(matches!(
        asked.resume().await,
        Err(Failure::Unavailable { .. })
    ));
    assert_eq!(asked.throttled().await.ok(), Client.throttled().await.ok());
    let wanted = Wanted {
        active: Rates {
            down: Some(3),
            up: Some(4),
        },
        quiet: Rates::default(),
        window: None,
    };
    assert_eq!(
        asked.restrain(&wanted).await.ok().map(|held| held.rates),
        Some(wanted.active)
    );
    assert_eq!(asked.moving().await.ok(), Client.moving().await.ok());
    assert_eq!(
        asked.moved("2026-10").await.ok().map(|moved| moved.down),
        Some(7)
    );
    assert_eq!(asked.seeding().await.ok(), Client.seeding().await.ok());
    assert_eq!(asked.stop_seeding("a film").await.ok(), Some(()));
    assert_eq!(asked.filed().await.ok(), Client.filed().await.ok());
    assert_eq!(asked.file_under(&tv_filed()).await.ok(), Some(()));
    assert_eq!(asked.peer_port().await.ok(), Some(Some(51413)));
    assert_eq!(asked.set_peer_port(Some(51413)).await.ok(), Some(()));
    assert_eq!(asked.set_peer_port(None).await.ok(), Some(()));
    assert!(matches!(
        asked.set_peer_port(Some(0)).await,
        Err(Failure::Refused { .. })
    ));
    assert!(matches!(
        asked.stop_seeding("another film").await,
        Err(Failure::Refused { detail, .. }) if detail.contains("another film")
    ));
}

/// Television filed under the one completed folder both clients share.
fn tv_filed() -> Vec<Filed> {
    vec![Filed {
        category: "tv".to_owned(),
        folder: "/data/downloads/complete/tv".to_owned(),
    }]
}

#[tokio::test]
async fn a_usenet_client_answers_its_accounts_and_not_seeding_or_a_peer_port() {
    let asked = usenet::Adapter(contracted(Served::default()));
    assert_eq!(asked.accounts().await.ok(), Some(Vec::new()));
    assert_eq!(asked.filed().await.ok(), Client.filed().await.ok());
    assert_eq!(asked.file_under(&tv_filed()).await.ok(), Some(()));
    assert!(matches!(
        asked
            .file_under(&[Filed {
                category: "tv".to_owned(),
                folder: String::new(),
            }])
            .await,
        Err(Failure::Refused { detail, .. }) if detail.contains("tv")
    ));
    for operation in ["seeding", "peer_port", "set_peer_port"] {
        let unserved = usenet::dispatch(&Client, operation, b"{}").await;
        assert_eq!(
            unserved.err().map(|refusal| refusal.kind),
            Some(wire::Kind::UnknownOperation),
            "{operation}"
        );
    }
}

#[tokio::test]
async fn an_operation_neither_capability_declares_is_refused_by_both() {
    for refused in [
        torrent::dispatch(&Client, "accounts", b"{}").await,
        usenet::dispatch(&Client, "nothing", b"{}").await,
    ] {
        assert_eq!(
            refused.err().map(|refusal| refusal.kind),
            Some(wire::Kind::UnknownOperation)
        );
    }
}

#[tokio::test]
async fn a_request_with_a_field_the_operation_does_not_take_is_not_asked() {
    let refused = torrent::dispatch(&Client, "moved", br#"{"month":"2026-10","more":1}"#).await;
    assert_eq!(
        refused.err().map(|refusal| refusal.kind),
        Some(wire::Kind::NotAsked)
    );
}

#[tokio::test]
async fn a_capability_nobody_serves_refuses_every_operation() {
    let asked: Result<(), Failure> = contracted(Served::default())
        .call("nothing.served", 1, "anything", crate::client::LARGEST, &())
        .await;
    assert!(matches!(asked, Err(Failure::Refused { .. })));
}

#[test]
fn every_operation_has_a_path_of_its_own_and_a_schema_each_way() {
    for capability in super::all() {
        let mut paths = HashSet::new();
        for operation in &capability.operations {
            assert!(paths.insert(operation.path()), "{} twice", operation.path());
            assert_eq!(operation.capability, capability.name);
            assert_eq!(operation.major, capability.major);
            assert!(operation.path().starts_with(&format!(
                "/lemonfiber/{}/v{}/",
                capability.name, capability.major
            )));
        }
    }
    let names: Vec<&str> = super::all()
        .iter()
        .map(|capability| capability.name)
        .collect();
    assert_eq!(
        names,
        [
            usenet::CAPABILITY,
            torrent::CAPABILITY,
            egress::CAPABILITY,
            curate::CAPABILITY,
            search::CAPABILITY,
            pull::CAPABILITY,
            fetch::CAPABILITY,
            intake::CAPABILITY,
            source::CAPABILITY,
            serve::CAPABILITY,
            front::CAPABILITY,
            sync::CAPABILITY,
            show::CAPABILITY,
            extract::CAPABILITY,
            guard::CAPABILITY,
            decline::CAPABILITY,
            gate::CAPABILITY,
            claim::CAPABILITY,
        ]
    );
}

#[tokio::test]
async fn every_operation_of_every_capability_refuses_a_body_that_is_not_its_request() {
    let served = Served::default();
    for capability in super::all() {
        for operation in &capability.operations {
            let request = Request {
                method: lemonfiber_ports::http::Method::Post,
                url: format!("http://adapter{}", operation.path()),
                headers: Vec::new(),
                body: Some("not a request".to_owned()),
                pinned: None,
            };
            let answered = served.send(&request).await.map(|response| response.status);
            assert_eq!(
                answered.ok(),
                Some(wire::Kind::NotAsked.status()),
                "{}",
                operation.path()
            );
        }
    }
}
