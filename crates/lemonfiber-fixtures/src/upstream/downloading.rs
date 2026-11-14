//! The download client the contract tests ask in process.

use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_ports::service::{
    Download, Failure, Fetching, Filed, Filing, Metering, Moved, Peering, Pulling, Rates, Seeded,
    Seeding, Throttled, Throttling, Transfers, UsenetAccount, UsenetAccounts, Wanted,
};

/// A download client that answers every question with something recognisable.
pub struct Downloader;

#[async_trait]
impl Transfers for Downloader {
    async fn transfers(&self) -> Result<Vec<Download>, Failure> {
        Ok(vec![Download {
            name: "a film".to_owned(),
            progress: 40,
            speed: Some(1),
            eta: Some(Duration::from_secs(60)),
            remaining: Some(9),
            id: Some("7a3f".to_owned()),
            category: Some("tv".to_owned()),
            state: Some(lemonfiber_ports::service::TransferState::Stalled),
            added: Some(1_799_000_000),
        }])
    }
}

#[async_trait]
impl Fetching for Downloader {
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
impl Throttling for Downloader {
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
impl Metering for Downloader {
    async fn moved(&self, month: &str) -> Result<Moved, Failure> {
        Ok(Moved {
            down: month.len() as u64,
            up: 0,
            since_start: false,
        })
    }
}

#[async_trait]
impl Seeding for Downloader {
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
impl Peering for Downloader {
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
impl Filing for Downloader {
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
impl UsenetAccounts for Downloader {
    async fn accounts(&self) -> Result<Vec<UsenetAccount>, Failure> {
        Ok(Vec::new())
    }
}
