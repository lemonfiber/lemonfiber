//! Where the room went, and which of it can be got back.
//!
//! "Forty gigabytes free" is the answer to a question nobody asked. The question
//! is what is using the rest, and the useful part of that answer is not the
//! largest line but the reclaimable one: a library is only reduced by deciding to
//! lose something, while a download nothing ever imported is pure waste and
//! usually a surprising amount of it.
//!
//! So every line carries what getting it back would cost, in the same words each
//! time. Three of the costs are nothing at all, one is a decision about content,
//! one is a tracker's opinion of you, and one is the operator's own instruction to
//! leave a thing alone — which is a cost too, and the one this product must never
//! quietly overrule.

use serde::Serialize;

use super::tally::Tally;

/// What one line of the accounting is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case", tag = "of", content = "name")]
#[schemars(rename = "SpaceCategory")]
pub enum Category {
    /// One directory beneath the data root, named as the operator named it.
    ///
    /// Per directory rather than one figure for the library, because several
    /// libraries commonly share a volume and "the library is large" tells nobody
    /// which of them is growing.
    Tree(String),
    /// What the download clients still have to write.
    Landing,
    /// Completed downloads the client is still seeding.
    Seeding,
    /// Downloads on disk that no service ever took.
    Orphaned,
    /// Archives whose extracted contents sit beside them.
    Extracted,
    /// The services' own configuration and databases.
    Services,
    /// What the operator said to leave alone.
    Unmanaged,
}

impl Category {
    /// How this line is headed.
    #[must_use]
    pub fn heading(&self) -> String {
        match self {
            Self::Tree(name) => name.clone(),
            Self::Landing => "still to land".to_owned(),
            Self::Seeding => "seeding".to_owned(),
            Self::Orphaned => "never imported".to_owned(),
            Self::Extracted => "archives already unpacked".to_owned(),
            Self::Services => "the services' own files".to_owned(),
            Self::Unmanaged => "left alone at your request".to_owned(),
        }
    }

    /// What getting this line's room back would cost.
    #[must_use]
    pub const fn reclaim(&self) -> Reclaim {
        match self {
            Self::Tree(_) => Reclaim::ByLosingContent,
            Self::Landing => Reclaim::InProgress,
            Self::Seeding => Reclaim::AtTheCostOfRatio,
            Self::Orphaned => Reclaim::TheEasyWin,
            Self::Extracted => Reclaim::AlreadyHaveIt,
            Self::Services => Reclaim::Marginally,
            Self::Unmanaged => Reclaim::YouSaidNot,
        }
    }
}

/// What getting a line's room back costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Reclaim {
    /// Only by deciding to lose something you chose to keep.
    ByLosingContent,
    /// Nothing to reclaim: it is being written right now.
    InProgress,
    /// Yes, and it costs standing with the trackers it came from.
    AtTheCostOfRatio,
    /// Yes, and it costs nothing — the usual easy win.
    TheEasyWin,
    /// Yes: the unpacked copy is the one being used.
    AlreadyHaveIt,
    /// A little, and rarely worth it.
    Marginally,
    /// No, because you said so.
    YouSaidNot,
}

impl Reclaim {
    /// What this costs, in the words it is always said in.
    #[must_use]
    pub const fn says(self) -> &'static str {
        match self {
            Self::ByLosingContent => "only by deciding to lose something",
            Self::InProgress => "nothing here — it is being written now",
            Self::AtTheCostOfRatio => "yes, at the cost of ratio with the tracker it came from",
            Self::TheEasyWin => "yes, and nothing is lost — nothing ever took these",
            Self::AlreadyHaveIt => "yes — the unpacked copy beside it is the one in use",
            Self::Marginally => "a little, and rarely worth it",
            Self::YouSaidNot => "no, because you asked for this one to be left alone",
        }
    }

    /// Whether lemonfiber will offer to reclaim this, rather than only say it is
    /// there.
    ///
    /// Two of the reclaimable ones are not offered, and for opposite reasons. A
    /// seeding torrent's removal has a consequence outside this machine that only
    /// the operator can weigh, so it is named and left with them. Something they
    /// asked to be left alone is not reclaimable at all until they say otherwise,
    /// which is the whole meaning of having asked.
    #[must_use]
    pub const fn offered(self) -> bool {
        matches!(self, Self::TheEasyWin | Self::AlreadyHaveIt)
    }
}

/// One line of the accounting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Consumption {
    /// What it is about.
    pub category: Category,
    /// What it occupies, counted both ways.
    pub tally: Tally,
    /// What getting it back would cost.
    pub reclaim: Reclaim,
}

impl Consumption {
    /// One line, taking its cost from what it is about rather than from a caller.
    ///
    /// The cost is a property of the category and never an argument, so two lines
    /// about the same kind of thing cannot come to disagree about what removing it
    /// would take.
    #[must_use]
    pub fn of(category: Category, tally: Tally) -> Self {
        Self {
            reclaim: category.reclaim(),
            category,
            tally,
        }
    }

    /// Whether this line accounts for anything at all.
    ///
    /// A line of nothing is left out rather than printed as zero: an operator
    /// looking for where their disk went is not helped by seven headings, six of
    /// which say nothing.
    #[must_use]
    pub const fn any(&self) -> bool {
        self.tally.physical > 0 || self.tally.files > 0
    }
}

#[cfg(test)]
mod tests;
