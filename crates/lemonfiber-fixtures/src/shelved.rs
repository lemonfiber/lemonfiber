//! A media server's library, answering whether it holds a title as a test scripts it.

use async_trait::async_trait;
use lemonfiber_ports::media::TitleKind;
use lemonfiber_ports::service::{Failure, Library};

/// A library answering every question about what it holds alike, or as one that cannot be
/// reached where it was given no answer.
pub struct Shelf(Option<bool>);

impl Shelf {
    /// A library answering whether it holds anything with `holds`, or not answering where
    /// that is nothing.
    #[must_use]
    pub const fn holding(holds: Option<bool>) -> Self {
        Self(holds)
    }
}

#[async_trait]
impl Library for Shelf {
    async fn has_item(&self, _kind: TitleKind, _term: &str) -> Result<bool, Failure> {
        self.0.ok_or_else(|| Failure::Unavailable {
            service: "jellyfin".to_owned(),
        })
    }

    async fn rescan(&self) -> Result<(), Failure> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
