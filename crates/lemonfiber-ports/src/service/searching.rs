//! Finding a title somebody may ask for, and asking for it on their behalf.
//!
//! What the request service knows of titles beyond the house: what a term finds, what one
//! title holds and is rated, and the one call that turns a member's wish into a request.
//! The core decides what a member is shown and may ask for; this port only answers.

use async_trait::async_trait;

use super::{Failure, MediaStatus};
use crate::media::Kind;

/// One title a search found.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Found {
    /// The id the request service knows the title by, which names it to every other call.
    pub id: String,
    /// Whether it is a series or a film.
    pub kind: Kind,
    /// What it is called.
    pub title: String,
    /// The year it came out, where the service knows one.
    pub year: Option<u16>,
    /// Where it stands in the house: here, partly here, on its way or unknown.
    pub status: MediaStatus,
    /// Where its poster is published, as the service gives it, or nothing.
    pub poster: Option<String>,
}

/// One page of what a search found.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Page {
    /// The titles on this page, in the service's order.
    pub titles: Vec<Found>,
    /// The page after this one, or nothing where this is the last.
    pub next: Option<u32>,
}

/// One season of a series, and where it stands in the house.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Season {
    /// Its number.
    pub number: u32,
    /// Where it stands in the house.
    pub status: MediaStatus,
}

/// What one title is, beyond what a search says of it.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Detail {
    /// What it is about, in the service's words, where it has any.
    pub overview: Option<String>,
    /// Its certification in the region asked for, or nothing where it has none there.
    pub certification: Option<String>,
    /// Whether it has come out yet.
    pub released: bool,
    /// Each season of a series, and none for a film.
    pub seasons: Vec<Season>,
}

/// What a member asks for: one title, and for a series the seasons of it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Wish {
    /// The id the request service knows the title by.
    pub id: String,
    /// Whether it is a series or a film.
    pub kind: Kind,
    /// The seasons asked for, and every season where none are named.
    pub seasons: Vec<u32>,
}

/// What became of an ask.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Asked {
    /// The number the request service files the request under.
    pub request: i64,
    /// Whether it waits for somebody to approve it.
    pub waiting: bool,
}

/// Finding titles beyond the house and asking for one on a member's behalf.
#[async_trait]
pub trait Searching: Send + Sync {
    /// One page of the titles of `kinds` that `term` finds, page `page` counting from 1.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn search(&self, term: &str, kinds: &[Kind], page: u32) -> Result<Page, Failure>;

    /// What the title `id` of `kind` is, with its certification in `region`, or nothing
    /// where the service knows no such title.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn detail(&self, kind: Kind, id: &str, region: &str) -> Result<Option<Detail>, Failure>;

    /// Ask for `wish` on behalf of the member the service holds as `member`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses; a refusal carries
    /// the service's own reason.
    async fn ask(&self, member: &str, wish: &Wish) -> Result<Asked, Failure>;
}
