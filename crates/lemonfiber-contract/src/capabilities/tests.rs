//! Each capability's client against its own dispatcher, through a transport that is
//! nothing but the two halves: what the core asks is what an adapter is handed, and what
//! an adapter answers is what the core reads.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_ports::http::{Http, Request, Response, Unreachable};
use lemonfiber_ports::service::{
    Download, Failure, Fetching, Metering, Moved, Pulling, Rates, Seeded, Seeding, Throttled,
    Throttling, Transfers, UsenetAccount, UsenetAccounts, Wanted,
};

use super::download::{torrent, usenet};
use super::identity::source;
use super::indexer::search;
use super::library::curate;
use super::media::serve;
use super::request::intake;
use super::subtitles::fetch;
use crate::{wire, Contracted};

mod identity;
mod indexer;
mod library;
mod media;
mod request;
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
            usenet::CAPABILITY => usenet::dispatch(&Client, operation, body).await,
            curate::CAPABILITY => curate::dispatch(&*self.upstream, operation, body).await,
            search::CAPABILITY => search::dispatch(&*self.upstream, operation, body).await,
            fetch::CAPABILITY => fetch::dispatch(&*self.upstream, operation, body).await,
            intake::CAPABILITY => intake::dispatch(&*self.upstream, operation, body).await,
            source::CAPABILITY => source::dispatch(&*self.upstream, operation, body).await,
            serve::CAPABILITY => serve::dispatch(&*self.upstream, operation, body).await,
            _ => Err(wire::Refusal::unknown_operation(operation)),
        };
        Ok(match served {
            Ok(body) => Response {
                status: 200,
                headers: Vec::new(),
                body: String::from_utf8(body).unwrap_or_default(),
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
}

#[tokio::test]
async fn a_usenet_client_answers_its_accounts_and_not_seeding() {
    let asked = usenet::Adapter(contracted(Served::default()));
    assert_eq!(asked.accounts().await.ok(), Some(Vec::new()));
    let unserved = usenet::dispatch(&Client, "seeding", b"{}").await;
    assert_eq!(
        unserved.err().map(|refusal| refusal.kind),
        Some(wire::Kind::UnknownOperation)
    );
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
            curate::CAPABILITY,
            search::CAPABILITY,
            fetch::CAPABILITY,
            intake::CAPABILITY,
            source::CAPABILITY,
            serve::CAPABILITY,
        ]
    );
}
