//! What a read was given, one carrier for every read.

use super::THE_STACK;

/// An item's identifier, standing for whichever title a request names.
const AN_ITEM: &str = "0123456789abcdef0123456789abcdef";

/// What a read was given, mirroring the flags its command takes.
///
/// One carrier rather than one shape per read, so a caller fills the field the read
/// names and leaves the rest. Each field is the query parameter of the same
/// meaning, taken as it was written — the parsing a season needs is done here, so
/// a request that named one badly is refused in the same words wherever it arrived.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// The forms to narrow to, or to say what starting would come to.
    pub forms: Vec<String>,
    /// The services of those forms to narrow to.
    pub services: Vec<String>,
    /// The household member to narrow to.
    pub member: Option<String>,
    /// Whether a household read is answered as the household's defaults, as it was
    /// written.
    pub defaults: Option<String>,
    /// What to follow, named as a person would say it.
    pub term: Option<String>,
    /// The season to narrow a trace to, as it was written.
    pub season: Option<String>,
    /// The setting to read, instead of every one of them.
    pub key: Option<String>,
    /// The group of checks, or the one check, to narrow a diagnosis to.
    pub only: Option<String>,
    /// The word to explain, instead of every word there is to ask about.
    pub word: Option<String>,
    /// Which of the four removals to read.
    pub tier: Option<String>,
    /// The version to move to, instead of whatever is newest.
    pub to: Option<String>,
    /// Which of the two things that can be moved forward is being read.
    pub what: Option<String>,
    /// How many holdings a shelf answers with.
    pub most: Option<String>,
    /// Where a page of a search said the next one begins.
    pub after: Option<String>,
    /// Whether the title a read is about is a series or a film, as it was written.
    pub kind: Option<String>,
    /// What a search for music or books looks for, as it was written.
    pub unit: Option<String>,
    /// The title a read is about, as its path names it.
    pub title: Option<String>,
}

impl Wanted {
    /// Enough for every read to reach the command it names: the subjects a read cannot
    /// be named without, and nothing that narrows one.
    ///
    /// For asking which command a request reaches without a request: what this carries
    /// is never read, and nothing about a stack is in it. Held to every read in a test,
    /// so a read that gains a required parameter is given one here too.
    #[must_use]
    pub fn naming_everything() -> Self {
        Self {
            member: Some("someone".to_owned()),
            term: Some("Sintel".to_owned()),
            title: Some(AN_ITEM.to_owned()),
            what: Some(THE_STACK.to_owned()),
            ..Self::default()
        }
    }
}
