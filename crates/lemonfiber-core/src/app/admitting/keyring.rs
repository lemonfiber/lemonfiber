//! The keys other programs hold, read afresh whenever one is presented, and when each was
//! last used.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime};

use tokio::sync::Mutex;

use crate::keys::{Kept, Record, Used};

/// How long after a key's use is written down another use of it is not.
const NOTED_WITHIN: Duration = Duration::from_secs(60);

/// What the record holds for a presented key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Holding {
    /// A key that admits, and its record.
    Active(Box<Record>),
    /// A key that was minted and no longer admits, or a record that does not read.
    Known,
    /// A secret nobody minted.
    Unknown,
}

/// When each key was last written down as used.
#[derive(Default)]
pub(crate) struct Keyring {
    noted: Mutex<HashMap<String, SystemTime>>,
}

impl Keyring {
    /// What the record kept at `kept` holds for `offered`.
    pub(crate) fn holding(kept: Option<&Path>, offered: &str) -> Holding {
        let Some(path) = kept else {
            return Holding::Unknown;
        };
        let Ok(kept) = Kept::at(path) else {
            return Holding::Known;
        };
        match kept.holding(offered) {
            Some(record) if record.is_revoked() => Holding::Known,
            Some(record) => Holding::Active(Box::new(record.clone())),
            None => Holding::Unknown,
        }
    }

    /// Write down at `used` that the key `name` was used at `now`, once a minute at most.
    pub(crate) async fn note(&self, used: Option<&Path>, name: &str, now: SystemTime) {
        let Some(path) = used else {
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
        let Some(at) = crate::instant::Instant::dated(now) else {
            return;
        };
        noted.insert(name.to_owned(), now);
        let mut used = Used::at(path);
        used.at.insert(name.to_owned(), at);
        used.keep(path);
    }
}

#[cfg(test)]
mod tests;
