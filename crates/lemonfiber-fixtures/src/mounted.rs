//! The drive a stack's data lives on, as each look at it finds it.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use lemonfiber_ports::filesystem::{Presence, Volume};

/// A drive answering each look with the next reading a test scripted.
pub struct Drive {
    readings: Vec<Presence>,
    /// What every look past the script finds, or the last reading again where unsaid.
    after: Option<Presence>,
    looked: AtomicUsize,
}

impl Drive {
    /// Each of `readings` in turn, then gone once they run out.
    #[must_use]
    pub fn playing(readings: Vec<Presence>) -> Self {
        Self::reading(readings, Some(Presence::Gone))
    }

    /// Each of `readings` in turn, then the last of them for good; gone where there are
    /// none.
    #[must_use]
    pub fn settling(readings: Vec<Presence>) -> Self {
        Self::reading(readings, None)
    }

    /// How many times it was looked at.
    #[must_use]
    pub fn looked(&self) -> usize {
        self.looked.load(Ordering::SeqCst)
    }

    fn reading(readings: Vec<Presence>, after: Option<Presence>) -> Self {
        Self {
            readings,
            after,
            looked: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Volume for Drive {
    async fn presence(&self, _path: &Path) -> Presence {
        let at = self.looked.fetch_add(1, Ordering::SeqCst);
        self.readings
            .get(at)
            .copied()
            .or(self.after)
            .or_else(|| self.readings.last().copied())
            .unwrap_or(Presence::Gone)
    }
}
