//! Standing notices the request service shows everybody who asks for something.
//!
//! Apart from [`Approving`](super::Approving) because that port answers about one person
//! — what they may ask for, what they have left, what becomes of one request they made.
//! This carries what is true of the house rather than of anybody in it, and it goes to
//! all of them at once because there is one page and everybody lands on it.
//!
//! **A notice is one sentence and nothing else.** Not a title with a body, not a level,
//! not an identifier: the surface a request service offers for this is a heading, so a
//! shape carrying more than a heading would be one whose extra halves no service could
//! be asked to show. What decides whether a notice is worth showing is decided before it
//! is written, not carried alongside it.
//!
//! **The order is the order they are shown in.** A list rather than a set, because two
//! notices in front of somebody deciding what to ask for are read top down and the one
//! that changes a mind belongs first.

use async_trait::async_trait;

use super::Failure;

/// Putting a standing notice where the household will read it before they ask.
///
/// The household has no account here and no way in, so anything they are owed has to
/// arrive through the service they already use. This is the part of that service which
/// can be made to carry words of this program's own.
#[async_trait]
pub trait Noticing: Send + Sync {
    /// Show exactly these, in this order, and take down any of this program's that are
    /// not among them.
    ///
    /// Only its own are touched. A notice somebody put there by hand is somebody else's
    /// sentence and is left exactly where they put it.
    ///
    /// One method rather than a read beside it: what is showing is read on the way to
    /// deciding whether anything needs writing, and a second door onto the same reading
    /// would be a second answer able to disagree with the one that acts.
    ///
    /// An empty list takes them all down, which is what a house with nothing to say
    /// wants: a notice left standing after it stopped being true is worse than one that
    /// was never shown, because somebody acts on it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn set_notices(&self, notices: &[String]) -> Result<(), Failure>;
}
