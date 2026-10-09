//! Each capability's client against its own dispatcher, through a transport that is
//! nothing but the two halves: what the core asks is what an adapter is handed, and what
//! an adapter answers is what the core reads.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_ports::http::{Http, Request, Response, Unreachable};
use lemonfiber_ports::service::{
    Download, Failure, Fetching, Metering, Moved, Pulling, Rates, Seeded, Seeding, Throttled,
    Throttling, Transfers, UsenetAccount, UsenetAccounts, Wanted,
};

use super::download::{torrent, usenet};
use crate::{wire, Contracted};

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

/// A transport that hands every call to one capability's dispatcher.
struct Served {
    torrent: bool,
}

#[async_trait]
impl Http for Served {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        let operation = request.url.rsplit('/').next().unwrap_or_default();
        let body = request.body.as_deref().unwrap_or_default().as_bytes();
        let served = if self.torrent {
            torrent::dispatch(&Client, operation, body).await
        } else {
            usenet::dispatch(&Client, operation, body).await
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

fn contracted(torrent: bool) -> Contracted {
    Contracted::new(
        Arc::new(Served { torrent }),
        "http://adapter",
        "adapter",
        "k",
    )
}

#[tokio::test]
async fn a_torrent_client_answers_through_its_contract_as_it_answers_in_process() {
    let asked = torrent::Client(contracted(true));
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
    let asked = usenet::Client(contracted(false));
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
    assert_eq!(names, vec!["download.usenet", "download.torrent"]);
}
