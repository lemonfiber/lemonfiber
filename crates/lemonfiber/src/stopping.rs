//! Whether to stop now or let the downloads finish.
//!
//! The operator typed `down`, so stopping is what they asked for and waiting is the
//! courtesy — which is why an unanswered question stops rather than waits. A prompt
//! that blocks a teardown by default would turn a stray keypress into a stack that
//! is still up an hour later, and the operator would have no reason to suspect it.
//!
//! Nobody is asked twice, and nobody is asked who cannot answer. A run with `--yes`,
//! or one whose input is not a terminal, has already said everything it is going to
//! say; putting a question to it would hang a script on a prompt it cannot see.

/// What is put to the operator when something is still coming down.
pub(crate) const ASK_TO_WAIT: &str = "Wait for them to finish before stopping? [y/N]";

/// What to do about downloads a stop would interrupt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choice {
    /// Stop now, downloads and all.
    Stop,
    /// Let them finish first.
    Wait,
}

/// Whether the operator still has to be asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Asking {
    /// They already said, or there is nobody to ask.
    Settled(Choice),
    /// Put the question to them.
    Ask,
}

/// What the flags and the terminal settle between them, before anybody is asked.
///
/// `--wait` is an answer, not a hint: an operator who typed it has decided, and
/// asking them again would be asking a question they have already answered.
pub(crate) const fn asking(wait: bool, yes: bool, present: bool) -> Asking {
    if wait {
        return Asking::Settled(Choice::Wait);
    }
    if yes || !present {
        return Asking::Settled(Choice::Stop);
    }
    Asking::Ask
}

/// What a typed answer means.
///
/// Only an explicit yes waits. Everything else — a no, a stray return, a word that
/// is neither — stops, because that is what was asked for and a misread answer
/// should land on the thing the operator typed rather than on the opposite of it.
pub(crate) fn answered(said: &str) -> Choice {
    match said.trim().to_lowercase().as_str() {
        "y" | "yes" => Choice::Wait,
        _ => Choice::Stop,
    }
}

#[cfg(test)]
mod tests {
    use super::{answered, asking, Asking, Choice, ASK_TO_WAIT};

    /// An operator who typed `--wait` has decided; asking again would be asking a
    /// question they have already answered.
    #[test]
    fn waiting_was_asked_for_so_nobody_is_asked() {
        assert_eq!(
            asking(true, false, true),
            Asking::Settled(Choice::Wait),
            "--wait decides it"
        );
        assert_eq!(
            asking(true, true, true),
            Asking::Settled(Choice::Wait),
            "--wait beats --yes, being the more specific of the two"
        );
    }

    /// A script has said everything it is going to say, and a prompt it cannot see
    /// would hang it.
    #[test]
    fn nobody_who_cannot_answer_is_asked() {
        assert_eq!(asking(false, true, true), Asking::Settled(Choice::Stop));
        assert_eq!(asking(false, false, false), Asking::Settled(Choice::Stop));
        assert_eq!(asking(false, true, false), Asking::Settled(Choice::Stop));
    }

    #[test]
    fn an_operator_at_a_terminal_is_asked() {
        assert_eq!(asking(false, false, true), Asking::Ask);
    }

    /// The default lands on what was typed. A prompt that waits unless told not to
    /// would turn a stray keypress into a stack still up an hour later.
    #[test]
    fn only_an_explicit_yes_waits() {
        for said in ["y", "Y", "yes", "YES", " yes "] {
            assert_eq!(answered(said), Choice::Wait, "{said:?}");
        }
        for said in ["", "n", "no", "\n", "later", "yep", "sure"] {
            assert_eq!(answered(said), Choice::Stop, "{said:?}");
        }
    }

    /// The question says which way the default falls, because the answer that
    /// interrupts a download should never be the one given by accident.
    #[test]
    fn the_question_shows_which_way_saying_nothing_goes() {
        assert!(ASK_TO_WAIT.contains("[y/N]"), "{ASK_TO_WAIT}");
    }
}
