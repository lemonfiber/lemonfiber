//! Reclaiming what the disk's account offers.
//!
//! Its own file beside the command that carries it, the way every other value a
//! command carries has one.

/// The offer being answered, and the categories of it to take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reclaiming {
    /// The offer being answered, as the run that made it named itself; without
    /// one, nothing is removed.
    pub agreement: Option<String>,
    /// The categories of what the offer holds as costing nothing to take, as a line's
    /// `of` spells them; all it holds where none is named.
    pub only: Vec<String>,
}
