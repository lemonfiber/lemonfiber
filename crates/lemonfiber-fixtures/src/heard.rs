//! A narrator that keeps every line it is told, for a test to read back.

use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_ports::narration::Narrator;

/// Everything a run said, and every line of a pull it handed on, each in its order.
#[derive(Debug, Default)]
pub struct Heard {
    /// What was said.
    said: Mutex<Vec<String>>,
    /// What a pull wrote.
    pulled: Mutex<Vec<String>>,
}

impl Heard {
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
        crate::noted(&self.said, said.to_owned());
    }

    async fn pulled(&self, line: &str) {
        crate::noted(&self.pulled, line.to_owned());
    }
}

#[cfg(test)]
mod tests;
