//! A narrator that keeps every line it is told, for a test to read back.

use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_ports::narration::Narrator;

/// What a narrator watching something answers as each line is said.
type Watched = Box<dyn Fn() -> bool + Send + Sync>;

/// Everything a run said, and every line of a pull it handed on, each in its order.
#[derive(Default)]
pub struct Heard {
    /// What was said.
    said: Mutex<Vec<String>>,
    /// What a pull wrote.
    pulled: Mutex<Vec<String>>,
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
}

#[async_trait]
impl Narrator for Heard {
    async fn say(&self, said: &str) {
        if let Some(watched) = &self.watching {
            crate::noted(&self.seen, watched());
        }
        crate::noted(&self.said, said.to_owned());
    }

    async fn pulled(&self, line: &str) {
        crate::noted(&self.pulled, line.to_owned());
    }
}

#[cfg(test)]
mod tests;
