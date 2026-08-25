//! What a keypress does to the view.
//!
//! Kept apart from what the view *is*, because they change for different reasons: a
//! key is added when the operator needs a way to do something, and the state changes
//! when the screen needs to know something new. Reading one had meant reading both.
//!
//! What a key means depends on whether a filter is being typed, and that decision is
//! made here rather than by the terminal on this module's behalf — which is why the
//! two `Press` and `Asked` vocabularies live beside the code that reads them.

use lemonfiber_core::logs::Level;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{Viewer, Words};

/// The severities the screen cycles through, in the order it offers them.
///
/// A list rather than a match on the level below, so that adding a rung is one edit
/// and there is no arm for a level nothing offers.
const RUNGS: [Option<Level>; 4] = [
    None,
    Some(Level::Info),
    Some(Level::Warn),
    Some(Level::Error),
];

/// What the operator asked for.
///
/// Deliberately close to the keyboard rather than to the screen: what a key means
/// depends on whether a filter is being typed, and that is a decision this module
/// makes rather than one the terminal should be making on its behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Press {
    /// A character, whatever the screen is doing.
    Typed(char),
    /// Rub out the last character of whatever is being typed.
    Rubout,
    /// Finish what is being typed and apply it.
    Accept,
    /// Give up on what is being typed, or leave the screen.
    Abandon,
    /// Further back into what has already happened.
    Back,
    /// Back towards the newest line.
    Forward,
    /// All the way to the newest line.
    Tail,
}

/// What a keypress asks this view for, or nothing for one it has no use for.
///
/// Beside the vocabulary it produces rather than in [`crate::terminal`], because
/// this view wants most characters as text where the dashboard wants two commands —
/// the difference is this module's to make, not the terminal's to make for it.
///
/// Ctrl-C is read as giving up rather than as the character it is: raw mode no
/// longer turns it into a signal, so an operator who reaches for it is asking to
/// back out — of a filter they were typing, or of the screen where they were not.
pub(crate) const fn wanted(key: KeyEvent) -> Option<Press> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Some(Press::Abandon),
            _ => None,
        };
    }
    match key.code {
        KeyCode::Char(character) => Some(Press::Typed(character)),
        KeyCode::Backspace => Some(Press::Rubout),
        KeyCode::Enter => Some(Press::Accept),
        KeyCode::Esc => Some(Press::Abandon),
        KeyCode::Up => Some(Press::Back),
        KeyCode::Down => Some(Press::Forward),
        KeyCode::End => Some(Press::Tail),
        _ => None,
    }
}

/// What a keypress asked for that the screen cannot do by itself.
///
/// Writing a file is the loop's to do, not the screen's — everything else here is
/// decided without touching anything outside this module, and an export would be the
/// one exception. So it is asked for rather than done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Asked {
    /// Nothing beyond what has already been done.
    Nothing,
    /// Write the view out.
    Export,
    /// Record the words just opened, since opening them is asking what they mean.
    Learned,
}

impl Viewer {
    /// Do what a keypress asks for.
    ///
    /// What was being typed is taken out of hand here, so entry ends unless the
    /// press puts it back. That way each arm below says plainly whether it is still
    /// a search being written, rather than leaving the mode to be reasoned about.
    pub(crate) fn pressed(&mut self, press: Press) -> Asked {
        match self.typing.take() {
            Some(typed) => {
                self.while_typing(typed, press);
                Asked::Nothing
            }
            None => self.while_reading(press),
        }
    }

    /// What a key means while a filter is being typed.
    ///
    /// Every printable character is text rather than a command, which is why the
    /// mode exists at all: an operator searching for `queue` should not have the `q`
    /// close the screen out from under them.
    fn while_typing(&mut self, mut typed: String, press: Press) {
        match press {
            Press::Typed(character) => {
                typed.push(character);
                self.typing = Some(typed);
            }
            Press::Rubout => {
                typed.pop();
                self.typing = Some(typed);
            }
            Press::Accept => {
                self.text = (!typed.is_empty()).then_some(typed);
                self.back_to_the_tail();
            }
            // What was typed goes and what was in force stays. Abandoning a search
            // should put the operator back where they were, not clear the filter
            // they had before they started typing a new one.
            Press::Abandon => (),
            Press::Back | Press::Forward | Press::Tail => self.typing = Some(typed),
        }
    }

    /// What a key means while the operator is reading.
    fn while_reading(&mut self, press: Press) -> Asked {
        match press {
            // The one key the screen cannot answer on its own.
            Press::Typed('e') => return Asked::Export,
            Press::Typed('q') | Press::Abandon => self.open = false,
            Press::Typed('/') => self.typing = Some(String::new()),
            Press::Typed('f') | Press::Tail => self.back_to_the_tail(),
            Press::Typed('?') => return self.show_or_hide_the_words(),
            Press::Typed('s') => self.next_service(),
            Press::Typed('w') => self.next_rung(),
            Press::Typed('c') => self.unfiltered(),
            Press::Typed(_) | Press::Accept | Press::Rubout => (),
            Press::Back => self.further_back(),
            Press::Forward => self.nearer_the_tail(),
        }
        Asked::Nothing
    }

    /// Show the words on the screen, or put them away.
    ///
    /// A run that explains nothing stays where it is: there is no fourth state in
    /// which they are shown anyway.
    fn show_or_hide_the_words(&mut self) -> Asked {
        self.words = match self.words {
            Words::Unexplained => Words::Unexplained,
            Words::Away => Words::Shown,
            Words::Shown => Words::Away,
        };
        // Opening them is the asking. Closing them again is not, and neither is a
        // key pressed on a run that explains nothing.
        if matches!(self.words, Words::Shown) {
            return Asked::Learned;
        }
        Asked::Nothing
    }

    /// Show the next service on its own, or all of them again at the end.
    fn next_service(&mut self) {
        self.service = match &self.service {
            None => self.seen.first().cloned(),
            Some(current) => self
                .seen
                .iter()
                .position(|name| name == current)
                .and_then(|at| self.seen.get(at + 1))
                .cloned(),
        };
        self.back_to_the_tail();
    }

    /// Ask for the next severity up, or for all of them again at the top.
    fn next_rung(&mut self) {
        let at = RUNGS
            .iter()
            .position(|rung| *rung == self.least)
            .unwrap_or(0);
        self.least = RUNGS.get(at + 1).copied().flatten();
        self.back_to_the_tail();
    }

    /// Put every filter back to showing everything.
    fn unfiltered(&mut self) {
        self.service = None;
        self.least = None;
        self.text = None;
        self.back_to_the_tail();
    }

    /// Further back into what has already happened, stopping at the oldest line.
    fn further_back(&mut self) {
        // Bounded by the oldest admitted line, which needs one more than the offset
        // already reached rather than a count of everything the filter allows.
        let reachable = self.held.latest(&self.filter(), self.back + 2).len();
        self.back = self.back.saturating_add(1).min(reachable.saturating_sub(1));
        self.told();
    }

    /// One line nearer the newest.
    fn nearer_the_tail(&mut self) {
        self.back = self.back.saturating_sub(1);
        self.told();
    }

    /// All the way to the newest line.
    fn back_to_the_tail(&mut self) {
        self.back = 0;
        self.told();
    }

    /// Tell the scrollback where the view now sits.
    ///
    /// The one place that happens, so the offset and what the screen says about it
    /// cannot come apart.
    fn told(&mut self) {
        if self.back == 0 {
            self.held.follow();
        } else {
            self.held.detach();
        }
    }
}
