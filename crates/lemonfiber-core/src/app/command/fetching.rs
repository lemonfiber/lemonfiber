//! The reads that answer raw bytes rather than a report.

use super::Whom;
use crate::collected::Unit;
use crate::ports::service::Picture;

/// A read answered with bytes and their media type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetching {
    /// A title's poster or backdrop, as the member it is asked for may see it.
    Title {
        /// Whose library the title is read from.
        member: Whom,
        /// The title.
        id: String,
        /// Which of its pictures.
        which: Picture,
    },
    /// The poster of something a member's search found, of the kind it was found as.
    Found {
        /// The member searching.
        member: String,
        /// What it was found as.
        kind: String,
        /// What it is called where it was found.
        id: String,
    },
    /// The cover of something a member's search found, of the unit it is collected in.
    Cover {
        /// The member searching.
        member: String,
        /// The unit it is collected in.
        unit: Unit,
        /// What it is called where it was found.
        id: String,
    },
    /// The cover of an ask the household keeps.
    Kept {
        /// The member asking, or nobody where it is the operator.
        member: String,
        /// The ask, by its reference.
        reference: String,
    },
    /// A support bundle this run kept, by its name.
    Bundle {
        /// Its file name.
        name: String,
    },
}
