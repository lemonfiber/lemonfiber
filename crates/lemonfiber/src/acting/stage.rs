//! Where the screen is between a keypress and what it came to.
//!
//! Every one of these is a state a frame can be drawn in, which is the whole of what
//! decides whether something belongs here. Being asked what a stack can be acted on
//! is not one: the asking and the answer are one statement in the loop, over a read
//! that does not await, so no frame is drawn between them — which is why the offer
//! waiting for that answer is a field on [`super::Acting`] rather than a stage.
//!
//! Three flows share the list at the top and the reading at the bottom. An action on
//! a key of its own is chosen a subject and then asked about; a question is taken off
//! a list and then answered; an errand is taken off another list, given a name where
//! it needs one, and shown what it would do before it is asked about. What each of
//! them decides is next door — in [`super::offer`], [`super::question`] and
//! [`super::errand`] — and what each of them says is in [`super::words`].

use super::chooser::Chooser;
use super::errand::Errand;
use super::offer::{Choice, Offer};
use super::question::Question;
use super::reading::Reading;

/// Where an action, a question or an errand stands.
pub(super) enum Stage {
    /// Nothing is open.
    Idle,
    /// Choosing what to act on.
    Choosing {
        /// The action being chosen for.
        offer: &'static Offer,
        /// What it can be given, one of them selected.
        chooser: Chooser<Choice>,
    },
    /// Holding the question before anything is done.
    Confirming {
        /// The action about to be taken.
        offer: &'static Offer,
        /// What it is about to be taken on.
        chosen: Choice,
    },
    /// The action is with the core.
    Running {
        /// The action that is running.
        offer: &'static Offer,
        /// What it is running on.
        chosen: Choice,
    },
    /// What it came to, until it is put away.
    Came(Reading),
    /// Choosing what to ask this stack.
    Wondering(Chooser<&'static Question>),
    /// Typing the word a question has to be given.
    Typing {
        /// The question waiting on it.
        question: &'static Question,
        /// What is asked for, above the line being typed.
        asks: &'static str,
        /// What has been typed of it.
        typed: String,
    },
    /// The question is with the core.
    Waiting(&'static Question),
    /// What it answered, until it is put away.
    Answered {
        /// The question that was asked.
        question: &'static Question,
        /// The answer, and where in it the box is.
        reading: Reading,
    },
    /// Choosing which of the rest of the errands to send this stack on.
    Sending(Chooser<&'static Errand>),
    /// Typing the name an errand has to be given.
    Naming {
        /// The errand waiting on it.
        errand: &'static Errand,
        /// What is asked for, above the line being typed.
        asks: &'static str,
        /// What has been typed of it.
        typed: String,
    },
    /// Asking the core what the errand would do, before anybody agrees to it.
    Weighing {
        /// The errand being weighed.
        errand: &'static Errand,
        /// The name it was given, or nothing where it takes none.
        typed: String,
    },
    /// Holding the question, with what it would do above it where the errand can say.
    Agreeing {
        /// The errand about to be sent.
        errand: &'static Errand,
        /// The name it was given, or nothing where it takes none.
        typed: String,
        /// What it would do, and where in it the box is, where it said.
        would: Option<Reading>,
    },
    /// The errand is with the core.
    Doing {
        /// The errand that is running.
        errand: &'static Errand,
        /// The name it was given, or nothing where it takes none.
        typed: String,
    },
}
