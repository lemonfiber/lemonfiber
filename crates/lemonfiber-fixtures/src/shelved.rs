//! A media server's library, answering whether it holds a title as a test scripts it.

use async_trait::async_trait;
use lemonfiber_ports::media::TitleKind;
use lemonfiber_ports::service::{Failure, Library};

/// A library answering every question about what it holds alike, or for titles of one kind
/// alone, or as one that cannot be reached where it was given no answer.
pub struct Shelf {
    holds: Option<bool>,
    only: Option<TitleKind>,
}

impl Shelf {
    /// A library answering whether it holds anything with `holds`, or not answering where
    /// that is nothing.
    #[must_use]
    pub const fn holding(holds: Option<bool>) -> Self {
        Self { holds, only: None }
    }

    /// A library holding titles of `kind` alone.
    #[must_use]
    pub const fn holding_only(kind: TitleKind) -> Self {
        Self {
            holds: Some(true),
            only: Some(kind),
        }
    }
}

#[async_trait]
impl Library for Shelf {
    async fn has_item(&self, kind: TitleKind, _term: &str) -> Result<bool, Failure> {
        self.holds
            .map(|holds| holds && self.only.is_none_or(|only| only == kind))
            .ok_or_else(|| Failure::Unavailable {
                service: "jellyfin".to_owned(),
            })
    }

    async fn rescan(&self) -> Result<(), Failure> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
