//! A narrator that keeps every line it is told, for a test to read back.

use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_ports::docker::LogLine;
use lemonfiber_ports::narration::{Arrival, Narrator};

/// What a narrator watching something answers as each line is said.
type Watched = Box<dyn Fn() -> bool + Send + Sync>;

/// Everything a run said, and every line of a pull it handed on, each in its order.
#[derive(Default)]
pub struct Heard {
    /// What was said.
    said: Mutex<Vec<String>>,
    /// What a pull wrote.
    pulled: Mutex<Vec<String>>,
    /// What each service wrote to its log, as it was handed on.
    logged: Mutex<Vec<String>>,
    /// Each arrival a member was told of: who, which ask, and its title.
    arrived: Mutex<Vec<[String; 3]>>,
    /// Each member let go of.
    removed: Mutex<Vec<String>>,
    /// What it asks as each line is said, where it watches something.
    watching: Option<Watched>,
    /// What that answered for each line said.
    seen: Mutex<Vec<bool>>,
}

impl Heard {
    /// A narrator asking `watched` as each line is said, so a test can tell what had
    /// already happened by the time it was.
    #[must_use]
    pub fn watching(watched: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self {
            watching: Some(Box::new(watched)),
            ..Self::default()
        }
    }

    /// Every line heard so far.
    ///
    /// A poisoned lock reads as nothing said, which fails a test that expected a line
    /// rather than passing one that expected none without having looked.
    #[must_use]
    pub fn said(&self) -> Vec<String> {
        self.said
            .lock()
            .map(|said| said.clone())
            .unwrap_or_default()
    }

    /// Every line heard so far, each with what the watched thing answered as it was said.
    #[must_use]
    pub fn said_while(&self) -> Vec<(String, bool)> {
        let seen = self
            .seen
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default();
        self.said().into_iter().zip(seen).collect()
    }

    /// Every line of a pull handed on so far.
    #[must_use]
    pub fn pull_lines(&self) -> Vec<String> {
        self.pulled
            .lock()
            .map(|pulled| pulled.clone())
            .unwrap_or_default()
    }

    /// Every arrival told so far: the member, the ask and the title.
    #[must_use]
    pub fn arrivals(&self) -> Vec<[String; 3]> {
        self.arrived
            .lock()
            .map(|arrived| arrived.clone())
            .unwrap_or_default()
    }

    /// Every member let go of so far.
    #[must_use]
    pub fn removals(&self) -> Vec<String> {
        self.removed
            .lock()
            .map(|removed| removed.clone())
            .unwrap_or_default()
    }

    /// Every log line handed on so far.
    #[must_use]
    pub fn log_lines(&self) -> Vec<String> {
        self.logged
            .lock()
            .map(|logged| logged.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Narrator for Heard {
    async fn say(&self, said: &str) {
        say(self, said);
    }

    async fn pulled(&self, line: &str) {
        crate::noted(&self.pulled, line.to_owned());
    }

    async fn logged(&self, line: &LogLine) {
        crate::noted(&self.logged, line.line.clone());
    }

    async fn arrived(&self, arrival: &Arrival<'_>) {
        crate::noted(
            &self.arrived,
            [arrival.member, arrival.ask, arrival.title].map(str::to_owned),
        );
    }

    async fn removed(&self, member: &str) {
        crate::noted(&self.removed, member.to_owned());
    }
}

/// How `Heard` answers `say`.
fn say(fake: &Heard, said: &str) {
    if let Some(watched) = &fake.watching {
        crate::noted(&fake.seen, watched());
    }
    crate::noted(&fake.said, said.to_owned());
}

#[cfg(test)]
mod tests;
