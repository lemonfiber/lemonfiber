//! What a member asks for when they look for something the house does not hold yet.

use crate::collected::Unit;
use crate::ports::service::Wish;

/// A member's search for titles beyond the house, and their ask for one of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wishing {
    /// One page of the titles a member's words find, under their limits.
    Find {
        /// The member searching, by name or id.
        member: String,
        /// The words searched for, as they were typed.
        term: String,
        /// Where a previous page said the next one begins, or nothing for the first.
        after: Option<String>,
        /// The album, artist, book, author or series looked for, or nothing for series
        /// and films.
        unit: Option<Unit>,
    },
    /// Ask for one title, or seasons of it, on a member's behalf.
    Ask {
        /// The member asking, by name or id.
        member: String,
        /// The title, its kind and the seasons asked for.
        wish: Wish,
    },
    /// Ask for one album, artist, book, author or series on a member's behalf.
    Collect {
        /// The member asking, by name or id.
        member: String,
        /// What is asked for, within music or books.
        unit: Unit,
        /// The curator's reference for it, as their search listed it.
        reference: String,
    },
}
