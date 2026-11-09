//! What a member finds when they look for something the house does not hold yet, and
//! what became of asking for it.
//!
//! Every word a member reads here is the core's, in the household's voice: nothing names
//! a service, a size, an address or how the stack is doing.

use serde::Serialize;

use super::MemberAsking;
use crate::collected::{Medium, Unit};
use crate::ports::media::Kind;
use crate::ports::service::{BookUnit, MusicUnit};

/// What kind of title a member's search and ask name: a series, a film, music or books.
///
/// The web surface's own name for what a member asks for, whose name an SDK already gives
/// to the kinds of envelope: the same words on the wire, under a name nothing else takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TitleKind {
    /// A series.
    Tv,
    /// A film.
    Movies,
    /// An album or a whole artist.
    Music,
    /// A book, a whole author or a series of books.
    Books,
}

impl TitleKind {
    /// What a household calls a title of this kind, one of them.
    #[must_use]
    pub const fn noun(self) -> &'static str {
        match self {
            Self::Tv => Kind::Tv.noun(),
            Self::Movies => Kind::Movies.noun(),
            Self::Music => "album and artist",
            Self::Books => "book, author and series",
        }
    }
}

impl From<Kind> for TitleKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Tv => Self::Tv,
            Kind::Movies => Self::Movies,
        }
    }
}

impl From<Medium> for TitleKind {
    fn from(medium: Medium) -> Self {
        match medium {
            Medium::Music => Self::Music,
            Medium::Books => Self::Books,
        }
    }
}

/// Whether a title a curator of video files is a series or a film, as a document that
/// names one of those two alone publishes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum VideoKind {
    /// A series.
    Tv,
    /// A film.
    Movies,
}

/// What one title of music or books is, within its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TitleUnit {
    /// One album.
    Album,
    /// A whole artist.
    Artist,
    /// One book.
    Book,
    /// A whole author.
    Author,
    /// A series of books.
    Series,
}

impl TitleUnit {
    /// What a household calls one.
    #[must_use]
    pub const fn noun(self) -> &'static str {
        match self {
            Self::Album => "album",
            Self::Artist => "artist",
            Self::Book => "book",
            Self::Author => "author",
            Self::Series => "book series",
        }
    }
}

impl From<Unit> for TitleUnit {
    fn from(unit: Unit) -> Self {
        match unit {
            Unit::Music(MusicUnit::Album) => Self::Album,
            Unit::Music(MusicUnit::Artist) => Self::Artist,
            Unit::Books(BookUnit::Book) => Self::Book,
            Unit::Books(BookUnit::Author) => Self::Author,
            Unit::Books(BookUnit::Series) => Self::Series,
        }
    }
}

/// Where a title a member's search found stands in the house.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Stands {
    /// All of it is here.
    Here,
    /// Some of it is here, and the rest may be asked for.
    PartlyHere,
    /// Somebody asked for it and it is coming.
    OnItsWay,
    /// It may be asked for.
    Askable,
}

/// One title a member's search found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Askable {
    /// The id it is asked for by.
    pub id: String,
    /// Whether it is a series, a film, music or books.
    pub kind: TitleKind,
    /// What it is within music or books, and nothing for a series or a film.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<TitleUnit>,
    /// What it is called.
    pub title: String,
    /// Its artist or author, where it is music or books and one is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// The year it came out, where that is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    /// Where its cover is published, where it is music or books and has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    /// Where it stands in the house.
    pub stands: Stands,
}

/// One page of what a member's search found, under their limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct AskableReport {
    /// The member the search was made for, by the name they are known by.
    pub member: String,
    /// The words searched for, trimmed.
    pub term: String,
    /// The kinds of title the house can fetch now, which are the kinds searched.
    pub kinds: Vec<TitleKind>,
    /// The titles on this page, in the order they were found.
    pub titles: Vec<Askable>,
    /// Where the next page begins, absent on the last.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    /// What the member may ask for: what their period allows and has left of each kind.
    pub asking: MemberAsking,
    /// Whether what they ask for waits for whoever runs the house, as one sentence.
    pub approval: String,
}

/// What asking for a title came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Became {
    /// Asked for, and on its way without anybody saying yes first.
    Asked,
    /// Asked for, and waiting for whoever runs the house to say yes.
    Waiting,
    /// Somebody already asked for it, so nothing new was asked for.
    OnItsWay,
}

/// What is left of a member's allowance once an ask is counted, for each kind a limit
/// holds them to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct LeftAfter {
    /// Films still to ask for, absent where nothing limits them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub films: Option<u32>,
    /// Seasons of television still to ask for, absent where nothing limits them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub television: Option<u32>,
    /// Albums and artists still to ask for, absent where nothing limits them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub music: Option<u32>,
    /// Books, authors and series still to ask for, absent where nothing limits them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub books: Option<u32>,
}

/// What asking for one title came to, or would come to under a rehearsal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct AskedReport {
    /// The member it was asked for, by the name they are known by.
    pub member: String,
    /// The id of the title asked for.
    pub title: String,
    /// Whether it is a series, a film, music or books.
    pub kind: TitleKind,
    /// What it is within music or books, and nothing for a series or a film.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<TitleUnit>,
    /// The seasons asked for, none for a film, music or books.
    pub seasons: Vec<u32>,
    /// The reference the ask is named by on the household list, where the core keeps it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    /// What it came to.
    pub outcome: Became,
    /// What is left of the member's allowance once it is counted.
    pub leaves: LeftAfter,
    /// Whether the title has come out yet. One that has not is asked for, and arrives
    /// once it is out.
    pub released: bool,
    /// All of it, in one household sentence.
    pub said: String,
    /// Whether this was a rehearsal: what would have happened, with none of it done.
    pub rehearsed: bool,
}

#[cfg(test)]
mod tests;
