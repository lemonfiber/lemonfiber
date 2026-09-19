//! What an invitation lets the person it is for watch.
//!
//! Its own file beside the command that carries it, the way every other value a
//! command carries has one. What makes it worth separating from the table is that it
//! is a decision rather than an argument: the two halves below are settled together or
//! not at all.

/// What an invitation lets the person it is for watch.
///
/// One value over two answers because they are one decision taken at one moment: which
/// libraries somebody may open and how far up the ratings they may go are the two
/// halves of what an account is *for*, and every surface asks both while it is asking
/// who the account is for.
///
/// Libraries are named the way the media server's own screens name them and are turned
/// into that server's own identifiers once, in the core, so no surface carries a table
/// of its own. The age limit needs no turning: the media server keeps it as a number
/// and the number is an age — see [`crate::age_limit`], which holds the words said for
/// one.
///
/// The default is the ordinary case and the one nobody has to think about: every
/// library, and no limit at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Allowance {
    /// The libraries they may open, by name. Empty is every one.
    pub libraries: Vec<String>,
    /// The age above which the media server holds things back. `None` is no limit.
    pub age_limit: Option<u32>,
    /// What is to happen to content the media server has no rating for. `None` leaves
    /// it to the default a restriction carries — see [`crate::app::invite`].
    pub unrated: Option<crate::ports::service::Unrated>,
}
