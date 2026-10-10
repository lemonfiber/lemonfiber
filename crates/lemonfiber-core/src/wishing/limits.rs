//! What one member may be offered, from what their media server account may watch.
//!
//! A title the request service finds carries no certification of its own, so for a
//! member held to anything each is looked up in the country the media server's rating
//! table is named for, and held against that table. Only a title with no certification
//! there is unrated, and the member's unrated policy decides it. One certified in words
//! the table does not name is held back, and a table or a country that cannot be read
//! offers nothing at all: a title is never let through because a lookup failed.

use lemonfiber_contract::capabilities::identity::source;

use crate::ports::service::{Access, Certificate, Failure, Unrated};

/// What one member may be offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Limits {
    /// The oldest certificate they may watch, where a limit holds them.
    age: Option<u32>,
    /// What becomes of a title with no rating.
    unrated: Unrated,
    /// The media server's own certificates and the ages it holds them at.
    certificates: Vec<Certificate>,
    /// The country those certificates are named for, empty where nothing limits the
    /// member and no certification is looked up.
    country: String,
}

impl Limits {
    /// The limits `access` holds a member to, with the table and the country they are
    /// read against where any limit holds them; nothing where either is needed and
    /// cannot be read, or the server names no certificates or no country.
    pub(super) async fn of(server: &dyn source::Fills, access: &Access) -> Option<Self> {
        if !held_back(access) {
            return Some(Self::open());
        }
        Self::read(access, server.ratings().await, server.country().await)
    }

    /// The limits nothing holds a member to.
    fn open() -> Self {
        Self {
            age: None,
            unrated: Unrated::LetThrough,
            certificates: Vec::new(),
            country: String::new(),
        }
    }

    /// The limits `access` holds a member to, read against what the server answered for
    /// its table and its country; nothing where either did not answer or named nothing.
    fn read(
        access: &Access,
        certificates: Result<Vec<Certificate>, Failure>,
        country: Result<Option<String>, Failure>,
    ) -> Option<Self> {
        let certificates = certificates
            .ok()
            .filter(|certificates| !certificates.is_empty())?;
        let country = country.ok().flatten()?;
        Some(Self {
            age: access.age_limit,
            unrated: access.unrated,
            certificates,
            country,
        })
    }

    /// Whether anything at all holds the member back.
    pub(super) fn restricted(&self) -> bool {
        self.age.is_some() || self.unrated == Unrated::HeldBack
    }

    /// The country a certification is looked up in, empty where nothing limits the
    /// member.
    pub(super) fn region(&self) -> &str {
        &self.country
    }

    /// Whether a title certified `certification` may be offered: a rated one at or under
    /// the member's limit, an unrated one where their policy lets it through, and never
    /// one certified in words the table does not name.
    pub(super) fn admits(&self, certification: Option<&str>) -> bool {
        let Some(certified) = certification else {
            return self.unrated == Unrated::LetThrough;
        };
        crate::rating::age_of(&self.certificates, certified)
            .is_some_and(|rated| self.age.is_none_or(|limit| rated <= limit))
    }
}

/// Whether `access` holds its member back from anything.
fn held_back(access: &Access) -> bool {
    access.age_limit.is_some() || access.unrated == Unrated::HeldBack
}

#[cfg(test)]
mod tests;
