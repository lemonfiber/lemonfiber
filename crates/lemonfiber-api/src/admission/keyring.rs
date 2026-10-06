//! The keys this surface admits, read afresh at every asking.
//!
//! A key is minted, listed and revoked by the core, often in another process — the
//! command line — so nothing about one is held here between requests. The record is
//! read at the moment a key is presented, which is what makes a revoke reach the key's
//! next request rather than this surface's next restart.
//!
//! The one thing written from here is when each key was last admitted. It goes in a
//! file of its own, so this surface never rewrites the record a mint or a revoke is
//! writing at the same moment, and at most once a minute for each key, so a program
//! polling every few seconds does not turn every request into a write.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use lemonfiber_core::keys::{Kept, Record, Used};
use tokio::sync::Mutex;

/// How long a last use is held before it is written down again.
const NOTED_WITHIN: Duration = Duration::from_secs(60);

/// What a presented secret turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Holding {
    /// A key this machine minted and has not revoked.
    Active(Record),
    /// A key this machine knows and refuses: revoked, or kept in a record that does not
    /// read. Not a guess, so not counted as one.
    Known,
    /// Nothing this machine minted.
    Unknown,
}

/// Where this surface reads its keys from, and what it has noted of their use.
#[derive(Default)]
pub struct Keyring {
    /// Where the keys are kept, where this machine keeps any.
    pub kept: Option<PathBuf>,
    /// Where each key's last use is written.
    pub used: Option<PathBuf>,
    /// When each key's use was last written down, so it is not written again within the
    /// minute.
    noted: Mutex<HashMap<String, SystemTime>>,
}

impl Keyring {
    /// Keys kept at `kept`, their use written to `used`.
    #[must_use]
    pub fn at(kept: Option<PathBuf>, used: Option<PathBuf>) -> Self {
        Self {
            kept,
            used,
            noted: Mutex::default(),
        }
    }

    /// What the secret `offered` is, as the record stands now.
    ///
    /// A record that cannot be read refuses every key without counting any of them: the
    /// keys in it were minted deliberately, and treating them as guesses would let a
    /// damaged file lock the operator out of their own password.
    #[must_use]
    pub fn holding(&self, offered: &str) -> Holding {
        let Some(path) = self.kept.as_deref() else {
            return Holding::Unknown;
        };
        let Ok(kept) = Kept::at(path) else {
            return Holding::Known;
        };
        match kept.holding(offered) {
            Some(record) if record.is_revoked() => Holding::Known,
            Some(record) => Holding::Active(record.clone()),
            None => Holding::Unknown,
        }
    }

    /// Write down that the key `name` was admitted at `now`, unless it was within the
    /// minute.
    pub async fn note(&self, name: &str, now: SystemTime) {
        let Some(path) = self.used.as_deref() else {
            return;
        };
        let mut noted = self.noted.lock().await;
        let recent = noted
            .get(name)
            .and_then(|then| now.duration_since(*then).ok())
            .is_some_and(|since| since < NOTED_WITHIN);
        if recent {
            return;
        }
        let Some(written) = lemonfiber_core::instant::written(now) else {
            return;
        };
        noted.insert(name.to_owned(), now);
        let mut used = Used::at(path);
        used.at.insert(name.to_owned(), written);
        used.keep(path);
    }
}

#[cfg(test)]
mod tests;
