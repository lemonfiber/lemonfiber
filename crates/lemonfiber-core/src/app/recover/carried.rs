//! What a reversal came to, kept apart from whether it is worth refusing over.

use super::refusing::{left_holding, needs_service, not_opened, not_put_back, to_replace};
use super::Undo;
use crate::error::Problem;

/// What a reversal came to, refused over the first change it left standing.
///
/// # Errors
///
/// Returns the [`Problem`] naming the changes of the first kind left standing: the
/// unopened, the operator's, the still holding, the secrets to replace, then the beyond
/// reach.
pub(crate) fn refused_over(carried: &Carried) -> Result<(), Box<Problem>> {
    if !carried.unread.is_empty() {
        return Err(Box::new(not_opened(&carried.unread)));
    }
    if !carried.theirs.is_empty() {
        return Err(Box::new(not_put_back(&carried.theirs)));
    }
    if !carried.still_holding.is_empty() {
        return Err(Box::new(left_holding(&carried.still_holding)));
    }
    if !carried.to_replace.is_empty() {
        return Err(Box::new(to_replace(&carried.to_replace)));
    }
    if carried.beyond_reach.is_empty() {
        Ok(())
    } else {
        Err(Box::new(needs_service(&carried.beyond_reach)))
    }
}

/// What a reversal came to, kept apart from whether it is worth refusing over.
///
/// The three ways a change can be left standing, each naming what was left. A reversal
/// asked for by name has to *report* them — an operator who asked for five things back
/// and got three needs to know which three — where the one a repair earns refuses over
/// the first of them it meets. Same work, two readings, so the reading is the caller's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Carried {
    /// The undos carried out, kept rather than counted so the report can name them.
    pub done: Vec<Undo>,
    /// Resources whose change only the service that made it can undo, where that
    /// service did not answer.
    pub beyond_reach: Vec<String>,
    /// Settings holding neither what the change wrote nor what putting it back would,
    /// so somebody has chosen them since and they were left exactly as they are.
    pub theirs: Vec<String>,
    /// Settings whose sealed record would not open, so there is nothing to put back.
    pub unread: Vec<String>,
    /// Directories still holding something this run did not put there.
    ///
    /// A directory lemonfiber made and something else has since filled is not one this
    /// reversal may take. Two plugins share the directory their documents sit in, and
    /// the first of them made it — so removing the first would take the second's
    /// document with it, or, once the operating system refuses, would stop the whole
    /// reversal over a directory whose only fault is that somebody else is using it.
    pub still_holding: Vec<String>,
    /// Secrets that must be replaced at the terminal: what they hold is not what the change
    /// left, or the earlier value is not kept.
    pub to_replace: Vec<String>,
}
