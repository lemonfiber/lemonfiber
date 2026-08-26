//! The one line at the foot of the screen, and what a run leaving now would wait for.
//!
//! The other surface this screen has. [`super`] decides what goes in the box over the
//! panels; this decides the row underneath them, which is a different job with a
//! different constraint: one line, on a screen whose width belongs to the panels, and
//! it has to say the most useful thing there is to say at that moment.
//!
//! **The keys give way to what is running rather than sitting beside it.** There is
//! one row. An operator who has just started something is owed what it is more than
//! they are owed a list they have just used, and the keys come back the moment it is
//! over.
//!
//! **And what a run leaving now would stay for is decided here too**, because it is
//! the same question asked once more on the way out: what is still with the core, and
//! whether it claimed the stack. The box says nothing about that — by then there is no
//! screen to draw one on — so the answer goes to the ordinary terminal instead, which
//! is why it is the one line here that is made safe for a terminal and never made to
//! fit a width.

use lemonfiber_core::text::plain;
use ratatui::text::Line;

use super::super::disturbing;
use super::super::lasting;
use super::super::offer::OFFERED;
use super::super::{errand, mending, quality, question, surface};
use super::shapes::dimmed;
use super::{doing, Stage, WATCHING};

/// The keys the screen answers whatever else is open.
const ALWAYS: &str = "q quit   r refresh   ? words";

/// How a guard is ended, and what leaving it does instead.
const GUARDING: &str = "esc lets it go   q closes the screen and leaves it guarding";

/// What is said beside a running action, and what leaving does about it.
///
/// There is no daemon behind this screen. The process drawing it is the one that
/// claimed the stack and issued the command, so leaving is not a tab being closed
/// on a server that carries on: the screen goes at once, and the run stays until
/// the action it started has finished and given the stack back.
const WAITING: &str = "still running   q closes the screen and waits for it";

/// The one line at the foot of the screen: the keys, or what is running.
///
/// The keys give way to the running action rather than sitting beside it. There is
/// one row, an operator who has just started something is owed what it is more than
/// they are owed a list they have just used, and the keys come back the moment it
/// is over.
pub(crate) fn footer(stage: &Stage, across: usize) -> Line<'static> {
    let said = match stage {
        Stage::Running { offer, taken } => {
            format!("{} {}   {WAITING}", offer.hint, taken.name())
        }
        Stage::Doing { errand, .. } => format!("{}   {WAITING}", errand.name),
        Stage::Applying { change, .. } => format!("{}   {WAITING}", change.name),
        Stage::Disturbing => format!("{}   {WAITING}", disturbing::NAME),
        Stage::Putting(mending) => format!("{}   {WAITING}", mending.name),
        // The one with no ending of its own says how it is ended; the one that ends
        // by itself says what leaving does, as every other running thing here does.
        Stage::Keeping {
            lasting,
            named,
            ends,
            ..
        } => format!(
            "{}   {}",
            doing(lasting, named),
            if *ends { GUARDING } else { WATCHING }
        ),
        _ => keys(),
    };
    dimmed(&said, across)
}

/// What a run leaving this screen now would stay for, or nothing where it may go.
///
/// Only what changes something. A read is with the core the same way and claims
/// nothing, so a screen left with one outstanding has nothing to stay for — which is
/// why this asks about [`Stage::Running`] and not about [`Stage::Waiting`]. A guard
/// that has already been let go has nothing to stay for either: it is no longer this
/// stage by then.
///
/// Said on the ordinary terminal once the screen is given back, where there is room
/// for the whole of it and no width to fit — so this is the one line here that goes
/// through [`plain`] alone rather than through [`shortened`].
pub(crate) fn staying_for(stage: &Stage) -> Option<String> {
    let said = match stage {
        Stage::Running { offer, taken } => waited(&format!("{} {}", offer.hint, taken.name())),
        Stage::Doing { errand, .. } => waited(errand.name),
        Stage::Applying { change, .. } => waited(change.name),
        // It takes the tunnel away on purpose and puts it back. A screen left in the
        // middle of that would leave the stack without one, which is exactly the
        // failure the check itself reports when the tunnel does not come back.
        // A repair reaches the services and proves itself by asking the check again.
        // A screen left in the middle of that has a stack halfway between two states
        // and nothing on it saying which.
        Stage::Putting(mending) => waited(mending.name),
        Stage::Disturbing => waited(disturbing::NAME),
        // The one that never ends by itself is the one this cannot say "to finish"
        // about. What it will go on doing, and the one thing that ends it once the
        // screen is gone, are said instead — a run held open on a promise nobody
        // explained is exactly what an operator reads as a hang.
        Stage::Keeping {
            lasting,
            named,
            ends: true,
            ..
        } => format!(
            "{} is still running — nothing more will happen until the data location is \
             lost, and Ctrl-C ends it",
            doing(lasting, named)
        ),
        // And this one does end, in minutes, but it does not reach the container
        // engine and claims nothing — so the reason to wait for it is that there is
        // nothing else to carry it, not that it holds the stack.
        Stage::Keeping { lasting, named, .. } => format!(
            "waiting for {} to finish — nothing else can carry it once this run has gone",
            doing(lasting, named)
        ),
        _ => return None,
    };
    Some(plain(&said))
}

/// What is said about a run left with something that claimed the stack to finish.
fn waited(doing: &str) -> String {
    format!("waiting for {doing} to finish — leaving it now would leave the stack claimed")
}

/// Every key this screen answers, in the order they are worth reading.
///
/// Built from the offers rather than written out, so an action added to the table
/// is an action the operator is told about.
fn keys() -> String {
    let mut said = vec![
        ALWAYS.to_owned(),
        format!("{} {}", question::KEY, question::HINT),
        format!("{} {}", errand::KEY, errand::HINT),
        format!("{} {}", lasting::KEY, lasting::HINT),
        format!("{} {}", quality::KEY, quality::HINT),
        format!("{} {}", mending::KEY, mending::HINT),
        format!("{} {}", surface::KEY, surface::HINT),
    ];
    said.extend(
        OFFERED
            .iter()
            .map(|offer| format!("{} {}", offer.key, offer.hint)),
    );
    said.join("   ")
}

#[cfg(test)]
mod tests {
    use super::{errand, keys, mending, OFFERED};
    use crate::acting::question::{HINT, KEY};

    /// Every action is on the footer, or the operator has no way to learn a key
    /// exists — a screen whose only account of what it can do is its source.
    #[test]
    fn the_footer_names_every_key_this_screen_answers() {
        let said = keys();

        assert!(said.contains("q quit"), "{said}");
        assert!(said.contains(&format!("{KEY} {HINT}")), "{said}");
        assert!(
            said.contains(&format!("{} {}", errand::KEY, errand::HINT)),
            "{said}"
        );
        assert!(
            said.contains(&format!("{} {}", mending::KEY, mending::HINT)),
            "{said}"
        );
        for offer in OFFERED {
            assert!(
                said.contains(offer.hint),
                "{} is missing: {said}",
                offer.hint
            );
            assert!(said.contains(offer.key), "{} is missing: {said}", offer.key);
        }
    }
}
