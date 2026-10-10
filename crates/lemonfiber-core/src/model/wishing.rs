//! What a member finds when they look for something the house does not hold yet, and
//! what became of asking for it.
//!
//! Every word a member reads here is the core's, in the household's voice: nothing names
//! a service, a size, an address or how the stack is doing.

use serde::Serialize;

use super::MemberAsking;
use crate::ports::media::Kind;

/// Whether a title is a series or a film, as a member's search and ask name it.
///
/// The web surface's own name for [`Kind`], whose name an SDK already gives to the kinds
/// of envelope: the same two words on the wire, under a name nothing else takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TitleKind {
    /// A series.
    Tv,
    /// A film.
    Movies,
}

impl From<Kind> for TitleKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Tv => Self::Tv,
            Kind::Movies => Self::Movies,
        }
    }
}

impl From<TitleKind> for Kind {
    fn from(kind: TitleKind) -> Self {
        match kind {
            TitleKind::Tv => Self::Tv,
            TitleKind::Movies => Self::Movies,
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
    /// Whether it is a series or a film.
    pub kind: TitleKind,
    /// What it is called.
    pub title: String,
    /// The year it came out, where that is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
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
}

/// What asking for one title came to, or would come to under a rehearsal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct AskedReport {
    /// The member it was asked for, by the name they are known by.
    pub member: String,
    /// The id of the title asked for.
    pub title: String,
    /// Whether it is a series or a film.
    pub kind: TitleKind,
    /// The seasons asked for, none for a film.
    pub seasons: Vec<u32>,
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
