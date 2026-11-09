//! What a household member asks a curator of music or of books for: an album or a whole
//! artist, a book, a whole author or a series, found by words in the curator's own
//! catalogue and asked for by the curator's reference for it.
//!
//! The core is the intake for both. It finds what a member may ask for here, decides what
//! the member may ask for under the household's policy, and asks the curator; this port
//! only answers.

use async_trait::async_trait;

use super::Failure;

/// What a member asks a curator of music for.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum MusicUnit {
    /// One album: a release group.
    Album,
    /// A whole artist: every album, and those that come later.
    Artist,
}

/// What a member asks a curator of books for.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum BookUnit {
    /// One book: a work.
    Book,
    /// A whole author: every book, and those that come later.
    Author,
    /// A series: every book of it, and those that come later.
    Series,
}

/// Where a candidate stands in the house.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateState {
    /// The house holds it.
    Here,
    /// The curator already wants it.
    Asked,
    /// Nothing has asked for it yet.
    MayAsk,
}

/// One album, artist, book, author or series a curator's catalogue found.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    /// The curator's reference for it, which names it to `request`.
    pub reference: String,
    /// What it is called.
    pub name: String,
    /// Its artist or author, where it is an album or a book.
    pub by: Option<String>,
    /// The year it first came out, where the curator knows one.
    pub year: Option<u16>,
    /// Where its cover is published, where it has one.
    pub cover: Option<String>,
    /// The format the household files a book in, for a book, an author or a series.
    pub format: Option<String>,
    /// Where it stands in the house.
    pub state: CandidateState,
}

/// One page of what a lookup found.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Candidates {
    /// What is on this page, in the curator's order.
    pub found: Vec<Candidate>,
    /// Where the next page begins, or nothing where this is the last.
    pub next: Option<String>,
}

/// One profile a request may name.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// The curator's id for it.
    pub id: i64,
    /// What it is called.
    pub name: String,
}

/// What a request may be made with.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct AskedWith {
    /// The root folders it files into.
    pub root_folders: Vec<String>,
    /// Its quality profiles.
    pub quality: Vec<Profile>,
    /// Its metadata profiles, which only a curator of music holds.
    pub metadata: Vec<Profile>,
}

/// A member's ask of a curator of music.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct MusicAsk {
    /// An album or a whole artist.
    pub unit: MusicUnit,
    /// The curator's reference for it.
    pub reference: String,
    /// The root folder it is filed under.
    pub root_folder: String,
    /// The quality profile it is fetched at.
    pub quality_profile: i64,
    /// The metadata profile it is catalogued with.
    pub metadata_profile: i64,
    /// Whether the curator searches for it once it wants it.
    pub search: bool,
}

/// A member's ask of a curator of books.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct BookAsk {
    /// A book, a whole author or a series.
    pub unit: BookUnit,
    /// The curator's reference for it.
    pub reference: String,
    /// The root folder it is filed under.
    pub root_folder: String,
    /// The quality profile it is fetched at.
    pub quality_profile: i64,
    /// Whether the curator searches for it once it wants it.
    pub search: bool,
}

/// What a member asks a curator of music for, found and asked for over `music.curate`.
#[async_trait]
pub trait MusicCurating: Send + Sync {
    /// The page of `unit`s `term` finds, beginning where `after` says, or at the first.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the curator is unreachable or refuses.
    async fn lookup(
        &self,
        unit: MusicUnit,
        term: &str,
        after: Option<&str>,
    ) -> Result<Candidates, Failure>;

    /// The `unit` `reference` names, as `lookup` would answer it, or nothing where the
    /// curator's catalogue holds no such reference.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the curator is unreachable or refuses.
    async fn candidate(
        &self,
        unit: MusicUnit,
        reference: &str,
    ) -> Result<Option<Candidate>, Failure>;

    /// Ask for `ask`, answering it as it now stands.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the curator is unreachable or refuses, a root folder,
    /// profile or reference it does not hold included.
    async fn request(&self, ask: &MusicAsk) -> Result<Candidate, Failure>;

    /// The root folders and profiles a request may name.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the curator is unreachable or refuses.
    async fn profiles(&self) -> Result<AskedWith, Failure>;
}

/// What a member asks a curator of books for, found and asked for over `books.curate`.
#[async_trait]
pub trait BookCurating: Send + Sync {
    /// The page of `unit`s `term` finds, beginning where `after` says, or at the first.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the curator is unreachable or refuses.
    async fn lookup(
        &self,
        unit: BookUnit,
        term: &str,
        after: Option<&str>,
    ) -> Result<Candidates, Failure>;

    /// The `unit` `reference` names, as `lookup` would answer it, or nothing where the
    /// curator's catalogue holds no such reference.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the curator is unreachable or refuses.
    async fn candidate(
        &self,
        unit: BookUnit,
        reference: &str,
    ) -> Result<Option<Candidate>, Failure>;

    /// Ask for `ask`, answering it as it now stands.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the curator is unreachable or refuses, a root folder,
    /// profile or reference it does not hold included.
    async fn request(&self, ask: &BookAsk) -> Result<Candidate, Failure>;

    /// The root folders and profiles a request may name.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the curator is unreachable or refuses.
    async fn profiles(&self) -> Result<AskedWith, Failure>;
}
