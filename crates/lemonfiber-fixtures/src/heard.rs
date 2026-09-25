//! A narrator that keeps every line it is told, for a test to read back.

use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_ports::narration::Narrator;

/// Everything a run said, in the order it said it.
#[derive(Debug, Default)]
pub struct Heard(Mutex<Vec<String>>);

impl Heard {
    /// Every line heard so far.
    ///
    /// A poisoned lock reads as nothing said, which fails a test that expected a line
    /// rather than passing one that expected none without having looked.
    #[must_use]
    pub fn said(&self) -> Vec<String> {
        self.0.lock().map(|said| said.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl Narrator for Heard {
    async fn say(&self, said: &str) {
        crate::noted(&self.0, said.to_owned());
    }
}

#[cfg(test)]
mod tests;
