//! An indexer aggregator the seed driver registers each curator in as an application, as
//! a test scripts it.

use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_ports::service::{AppSync, Application, Failure, RegisteredApplication};

use crate::receiving::{down, Mode};

/// An indexer aggregator that answers the app-sync driver from a script.
pub struct Applications {
    mode: Mode,
    held: Mutex<Vec<RegisteredApplication>>,
    reads: Mutex<u32>,
    next_id: Mutex<u32>,
    /// Whether the key it stores is one the curator no longer answers to.
    stale: Mutex<bool>,
    /// Whether a new key it is given still fails its test.
    stays_stale: bool,
    /// Every key it was given, in order.
    rekeyed: Mutex<Vec<String>>,
    /// Whether it refuses to be given a key at all.
    refuses_rekey: bool,
}

impl Applications {
    /// An aggregator answering in this mode, already holding these applications.
    #[must_use]
    pub fn with(mode: Mode, applications: Vec<RegisteredApplication>) -> Self {
        Self {
            mode,
            held: Mutex::new(applications),
            reads: Mutex::new(0),
            next_id: Mutex::new(100),
            stale: Mutex::new(false),
            stays_stale: false,
            rekeyed: Mutex::new(Vec::new()),
            refuses_rekey: false,
        }
    }

    /// An aggregator answering normally, holding these applications under a key the curator
    /// has since replaced; where `stays_stale`, a new key it is given fails too.
    #[must_use]
    pub fn stale(applications: Vec<RegisteredApplication>, stays_stale: bool) -> Self {
        Self {
            stale: Mutex::new(true),
            stays_stale,
            ..Self::with(Mode::Normal, applications)
        }
    }

    /// The same aggregator, refusing to be given a key at all.
    #[must_use]
    pub fn refusing_rekey(self) -> Self {
        Self {
            refuses_rekey: true,
            ..self
        }
    }

    /// Every key it was given, in order.
    #[must_use]
    pub fn rekeyed(&self) -> Vec<String> {
        self.rekeyed
            .lock()
            .map(|keys| keys.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl AppSync for Applications {
    async fn register_application(&self, application: &Application) -> Result<(), Failure> {
        match self.mode {
            Mode::Down => Err(down("prowlarr")),
            Mode::RejectsRegister => Err(Failure::Refused {
                service: "prowlarr".to_owned(),
                detail: "HTTP 400: unknown implementation".to_owned(),
            }),
            Mode::Swallows => Ok(()),
            _ => {
                if let (Ok(mut applications), Ok(mut id)) = (self.held.lock(), self.next_id.lock())
                {
                    applications.push(RegisteredApplication {
                        id: id.to_string(),
                        base_url: application.base_url.trim_end_matches('/').to_owned(),
                    });
                    *id += 1;
                }
                Ok(())
            }
        }
    }

    async fn test_application(&self, _held: &RegisteredApplication) -> Result<(), Failure> {
        if self.stale.lock().map_or(true, |stale| *stale) {
            return Err(Failure::Refused {
                service: "prowlarr".to_owned(),
                detail: "HTTP 400: cannot connect to Sonarr".to_owned(),
            });
        }
        Ok(())
    }

    async fn rekey_application(
        &self,
        _held: &RegisteredApplication,
        key: &str,
    ) -> Result<(), Failure> {
        if self.refuses_rekey {
            return Err(Failure::Refused {
                service: "prowlarr".to_owned(),
                detail: "HTTP 400: validation failed".to_owned(),
            });
        }
        if let Ok(mut rekeyed) = self.rekeyed.lock() {
            rekeyed.push(key.to_owned());
        }
        if let Ok(mut stale) = self.stale.lock() {
            *stale = self.stays_stale;
        }
        Ok(())
    }

    async fn applications(&self) -> Result<Vec<RegisteredApplication>, Failure> {
        let count = match self.reads.lock() {
            Ok(mut reads) => {
                *reads += 1;
                *reads
            }
            Err(_) => 0,
        };
        match self.mode {
            Mode::Down => Err(down("prowlarr")),
            Mode::RefusesList => Err(Failure::Unauthorised {
                service: "prowlarr".to_owned(),
            }),
            Mode::DropsAfterRegister if count >= 2 => Err(down("prowlarr")),
            _ => Ok(self
                .held
                .lock()
                .map(|applications| applications.clone())
                .unwrap_or_default()),
        }
    }
}

#[cfg(test)]
mod tests;
