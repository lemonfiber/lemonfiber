//! A download client holding the folder each category files under.

use std::collections::BTreeMap;
use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Filed, Filing};

/// The name a client that does not answer, or refuses, is reported under.
const SERVICE: &str = "sabnzbd";

/// A client holding each category's folder in memory, recording every folder it is told
/// to file under, and scripted to refuse a write, ignore one, or stop answering reads.
#[derive(Default)]
pub struct Filer {
    held: Mutex<BTreeMap<String, String>>,
    told: Mutex<Vec<Filed>>,
    reads: Mutex<usize>,
    refusing: bool,
    ignoring: bool,
    answering: Option<usize>,
}

impl Filer {
    /// A client already filing each category under the folder beside it.
    #[must_use]
    pub fn with(held: &[(&str, &str)]) -> Self {
        Self {
            held: Mutex::new(
                held.iter()
                    .map(|(category, folder)| ((*category).to_owned(), (*folder).to_owned()))
                    .collect(),
            ),
            ..Self::default()
        }
    }

    /// The same client, refusing every write.
    #[must_use]
    pub fn refusing(self) -> Self {
        Self {
            refusing: true,
            ..self
        }
    }

    /// The same client, taking every write and filing nothing it is told.
    #[must_use]
    pub fn ignoring(self) -> Self {
        Self {
            ignoring: true,
            ..self
        }
    }

    /// The same client, answering only this many reads and not answering after.
    #[must_use]
    pub fn answering(self, reads: usize) -> Self {
        Self {
            answering: Some(reads),
            ..self
        }
    }

    /// Every folder it was told to file under, in order.
    #[must_use]
    pub fn told(&self) -> Vec<Filed> {
        self.told
            .lock()
            .map(|told| told.clone())
            .unwrap_or_default()
    }

    /// The folder it files `category` under, where it holds one.
    #[must_use]
    pub fn holds(&self, category: &str) -> Option<String> {
        self.held
            .lock()
            .ok()
            .and_then(|held| held.get(category).cloned())
    }
}

#[async_trait]
impl Filing for Filer {
    async fn filed(&self) -> Result<Vec<Filed>, Failure> {
        let read = self
            .reads
            .lock()
            .map(|mut reads| {
                *reads += 1;
                *reads
            })
            .unwrap_or_default();
        if self.answering.is_some_and(|answered| read > answered) {
            return Err(Failure::Unavailable {
                service: SERVICE.to_owned(),
            });
        }
        Ok(self
            .held
            .lock()
            .map(|held| {
                held.iter()
                    .map(|(category, folder)| Filed {
                        category: category.clone(),
                        folder: folder.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn file_under(&self, folders: &[Filed]) -> Result<(), Failure> {
        if let Ok(mut told) = self.told.lock() {
            told.extend_from_slice(folders);
        }
        if self.refusing {
            return Err(Failure::Refused {
                service: SERVICE.to_owned(),
                detail: "no".to_owned(),
            });
        }
        if !self.ignoring {
            if let Ok(mut held) = self.held.lock() {
                for one in folders {
                    held.insert(one.category.clone(), one.folder.clone());
                }
            }
        }
        Ok(())
    }
}
