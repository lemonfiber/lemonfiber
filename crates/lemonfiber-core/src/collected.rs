//! The asks for music and books the core keeps itself, since no request service holds them.
//!
//! Each is named on the household list by a reference of its own, `music-<n>` or
//! `books-<n>`, beside the request service's bare numbers, so a decision can never land
//! on the wrong one. An ask for a whole artist, a whole author or a series always waits
//! for the operator; an album or a book follows the household's policy.

pub(crate) mod store;

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ports::service::{BookUnit, MusicUnit};

/// A medium the core keeps asks for.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Medium {
    /// Albums and artists, asked of a curator speaking `music.curate`.
    Music,
    /// Books, authors and series, asked of a curator speaking `books.curate`.
    Books,
}

impl Medium {
    /// Both, music first.
    pub const ALL: [Self; 2] = [Self::Music, Self::Books];

    /// The word its references begin with, which is also its kind on the household list.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Music => "music",
            Self::Books => "books",
        }
    }
}

/// What one ask is for, within its medium.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "medium", content = "unit", rename_all = "snake_case")]
pub enum Unit {
    /// An album or a whole artist.
    Music(MusicUnit),
    /// A book, a whole author or a series.
    Books(BookUnit),
}

impl Unit {
    /// The medium it is of.
    #[must_use]
    pub const fn medium(self) -> Medium {
        match self {
            Self::Music(_) => Medium::Music,
            Self::Books(_) => Medium::Books,
        }
    }

    /// Whether an ask for it waits for the operator whatever the household's policy: a
    /// whole artist, a whole author or a series does, an album or a book does not.
    #[must_use]
    pub const fn always_waits(self) -> bool {
        matches!(
            self,
            Self::Music(MusicUnit::Artist) | Self::Books(BookUnit::Author | BookUnit::Series)
        )
    }
}

/// A request on the household list, by the reference it is named with there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reference {
    /// One the request service holds, by its own number.
    Filed(i64),
    /// One the core keeps, by its medium and its number among that medium's asks.
    Kept(Medium, u64),
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filed(number) => write!(f, "{number}"),
            Self::Kept(medium, number) => write!(f, "{}-{number}", medium.word()),
        }
    }
}

/// A reference that names no request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotAReference;

impl FromStr for Reference {
    type Err = NotAReference;

    fn from_str(written: &str) -> Result<Self, Self::Err> {
        let written = written.trim();
        if let Some((word, number)) = written.split_once('-') {
            let medium = Medium::ALL
                .into_iter()
                .find(|medium| medium.word() == word)
                .ok_or(NotAReference)?;
            return digits(number)
                .and_then(|number| number.parse().ok())
                .filter(|number| *number > 0)
                .map(|number| Self::Kept(medium, number))
                .ok_or(NotAReference);
        }
        digits(written)
            .and_then(|number| number.parse().ok())
            .map(Self::Filed)
            .ok_or(NotAReference)
    }
}

/// `written`, where it is one or more ASCII digits and nothing else.
fn digits(written: &str) -> Option<&str> {
    (!written.is_empty() && written.bytes().all(|byte| byte.is_ascii_digit())).then_some(written)
}

#[cfg(test)]
mod tests;
