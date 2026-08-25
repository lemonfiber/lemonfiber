//! Handing the terminal to the other surface.
//!
//! The one request on this screen that is not work sent to the stack. Everything
//! else here reaches one of the core's commands and is answered; this reaches none,
//! because no other surface has an action for it — a surface cannot start itself, so
//! there is nothing on the web to name and nothing to translate. It is a key rather
//! than an entry on either list for that reason: it belongs beside `q`, which is the
//! other key that ends this screen, and not beside things that are run on the stack.
//!
//! **It ends the screen rather than sharing it.** The web surface announces an
//! address, the warning that the connection is not encrypted, and the token every
//! request to it must carry — eleven lines an operator has to be able to read, copy
//! and come back to. Printed over an alternate screen that is about to be torn down
//! they would be gone; drawn into a box they could not be copied out of a terminal in
//! raw mode. So the screen is given back first and the surface takes an ordinary
//! terminal, where the announcement has room and Ctrl-C means what it says it does.
//!
//! **The question is asked because leaving is what it does.** Nothing on this screen
//! happens on one keypress, and this one is not an exception: what it costs is the
//! dashboard, and an operator who reached for the wrong letter should not lose the
//! screen they were reading.

use super::{Press, Stage, Wanted};

/// The key that starts the web surface.
pub(crate) const KEY: char = 'w';

/// The word the footer puts beside that key.
pub(crate) const HINT: &str = "web";

/// The question, which names what it costs before it costs it.
pub(crate) const ASKS: &str = "Close this screen and start the web interface";

/// What that comes to, in the line under the question.
pub(crate) const ABOUT: &str =
    "it serves to this machine only, and says its address and the word it will ask you for";

/// At the question: only an explicit yes goes ahead.
pub(super) fn handing(stage: &mut Stage, press: &Press) -> Wanted {
    if !matches!(*press, Press::Typed('y' | 'Y')) {
        return Wanted::Nothing;
    }
    *stage = Stage::Idle;
    Wanted::Serve
}

#[cfg(test)]
mod tests {
    use super::{ABOUT, ASKS, KEY};
    use crate::acting::{Acting, Press, Wanted};
    use lemonfiber::reaching::OPENS;

    /// The key is not one the screen already answers, or the thing it already did
    /// stops happening and nothing says so.
    #[test]
    fn the_key_is_not_one_the_screen_already_answers() {
        for taken in [
            'q',
            'r',
            '?',
            'y',
            crate::acting::question::KEY,
            crate::acting::errand::KEY,
            crate::acting::lasting::KEY,
        ] {
            assert_ne!(KEY, taken, "{taken:?} was already spoken for");
        }
        for offer in crate::acting::offer::OFFERED {
            assert_ne!(KEY, offer.key, "{:?} was already spoken for", offer.key);
        }
    }

    /// The question says what it costs, since what it costs is the screen being read.
    #[test]
    fn the_question_says_that_the_screen_goes() {
        assert!(ASKS.contains("Close this screen"));
        assert!(!ABOUT.is_empty());
    }

    /// Nothing happens on one keypress here either, and what a no leaves behind is
    /// the screen that was being read.
    #[test]
    fn only_an_explicit_yes_hands_the_terminal_over() {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(KEY));

        assert_eq!(acting.pressed(&Press::Typed('n')), Wanted::Nothing);
        assert_eq!(acting.pressed(&Press::Typed(KEY)), Wanted::Nothing);
        assert_eq!(acting.pressed(&Press::Typed('Y')), Wanted::Serve);
    }

    /// The one thing this key is checked against from outside the binary. It reaches
    /// no action and no read, so there is no table of another surface's to hold the
    /// parity row against — the join is the key itself doing what the row claims,
    /// and the published list saying so. Without both, the row would be the only
    /// unheld cell in that column again.
    #[test]
    fn the_request_this_key_reaches_is_published_for_the_parity_table() {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(KEY));

        assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Serve);
        assert_eq!(OPENS, ["ui"]);
    }
}
