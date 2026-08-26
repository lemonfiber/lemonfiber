//! What each box on this screen is called, on its own border.
//!
//! One of the two things a box is — [`super::pane`] decides what it *says*, and this
//! decides what it is *called* — kept apart because they are answered from different
//! places. What a box says is built from where the flow stands and how much room
//! there is; what it is called is the one word the thing that opened it already goes
//! by, or a fixed word for the lists that are about no one thing.
//!
//! **A second list is not a second thing.** Where a flow opens one box after another
//! — the services inside the forms an action was given — the second carries the name
//! of what opened the first. A title of its own there would read as a second thing
//! having been started, which is exactly what it is not.

use super::super::errand::Errand;
use super::super::lasting::Lasting;
use super::super::mending::Mending;
use super::super::offer::Offer;
use super::super::quality::Change;
use super::super::question::Question;
use super::super::service::Inside;

/// What the box holding a report calls itself.
pub(super) const CAME: &str = " what it came to ";

/// What the box holding the questions calls itself.
pub(super) const ASK: &str = " ask ";

/// What the box holding the rest of the errands calls itself.
pub(super) const MORE: &str = " more ";

/// What the box holding the two that keep going calls itself.
pub(super) const KEEPS_GOING: &str = " keeps going ";

/// What the box asking about the web surface calls itself.
pub(super) const WEB: &str = " web interface ";

/// What the box holding the three quality changes calls itself.
pub(super) const QUALITY: &str = " quality ";

/// What the box holding the list of what to do about a diagnosis is called.
pub(super) const PUT_RIGHT: &str = " put right ";

/// What the box holding one action is called.
pub(super) fn titled(offer: &Offer) -> String {
    named(offer.hint)
}

/// What the box holding the services inside what was named is called.
///
/// The action or the errand they are being named for, rather than a name of its own.
pub(super) fn narrowing(inside: &Inside) -> String {
    match *inside {
        Inside::Action { offer, .. } => titled(offer),
        Inside::Errand(errand) => sending(errand),
    }
}

/// What the box holding one question's answer is called.
pub(super) fn asked(question: &Question) -> String {
    named(question.name)
}

/// What the box holding one errand is called.
pub(super) fn sending(errand: &Errand) -> String {
    named(errand.name)
}

/// What the box holding one of the two writes about a diagnosis is called.
pub(super) fn righting(mending: &Mending) -> String {
    named(mending.name)
}

/// What the box holding one quality change is called.
pub(super) fn changing(change: &Change) -> String {
    named(change.name)
}

/// What the box holding one of the two that keep going is called.
pub(super) fn keeping(lasting: &Lasting) -> String {
    named(lasting.name)
}

/// One name, spaced off the border it is drawn on.
///
/// Seven callers, one spelling. A title drawn without the spaces sits against the
/// corner of its own box, and a screen where six agree and one does not is a screen
/// somebody has to look twice at.
fn named(name: &str) -> String {
    format!(" {name} ")
}
