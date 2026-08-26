//! Where the screen is between a keypress and what it came to.
//!
//! Every one of these is a state a frame can be drawn in, which is the whole of what
//! decides whether something belongs here. Being asked what a stack can be acted on
//! is not one: the asking and the answer are one statement in the loop, over a read
//! that does not await, so no frame is drawn between them — which is why the offer
//! waiting for that answer is a field on [`super::Acting`] rather than a stage.
//!
//! Five flows share the list at the top and the reading at the bottom. An action on
//! a key of its own is chosen a subject and then asked about; a question is taken off
//! a list and then answered, or answered with a list of its own and taken off that
//! too; an errand is taken off another list, given a name where it needs one, and
//! shown what it would do before it is asked about; one of the
//! two that keep going is taken off a third, told what to look for or chosen a form,
//! and then watched rather than waited for; and a quality change is taken off a
//! fourth, chosen a preset or asked what it would cost, and then asked about. What
//! each of them decides is next door — in [`super::offer`], [`super::question`],
//! [`super::errand`], [`super::lasting`] and [`super::quality`] — and what each of
//! them says is in [`super::words`].
//!
//! The sixth key has one stage and no flow: [`super::surface`] asks whether to hand
//! the terminal over, and a yes ends the screen rather than beginning anything here.

use super::chooser::Chooser;
use super::errand::Errand;
use super::lasting::{Begun, Lasting};
use super::narrowing::Subject;
use super::offer::{Choice, Offer, Taken};
use super::quality::{Change, Grade};
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
        /// What it is about to be taken on: one subject, or the forms marked together.
        taken: Taken,
    },
    /// The action is with the core.
    Running {
        /// The action that is running.
        offer: &'static Offer,
        /// What it is running on.
        taken: Taken,
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
    /// One of the things the question listed is with the core.
    ///
    /// Apart from [`Stage::Waiting`] because what comes back is a different shape:
    /// there it is the listing to take one of, here it is the answer about the one
    /// taken. One stage for both would have to guess which, and it would guess wrong
    /// exactly once per narrowing — on the answer.
    Following(&'static Question),
    /// Choosing which of the things a question listed it is really about.
    Narrowing {
        /// The question being narrowed, which is what the box is titled by.
        question: &'static Question,
        /// The entries it listed, one of them selected.
        chooser: Chooser<Subject>,
    },
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
    /// Choosing which of the two that keep going to start.
    Starting(Chooser<&'static Lasting>),
    /// Typing what a walk is to look for.
    Wording {
        /// The walk waiting on it.
        lasting: &'static Lasting,
        /// What is asked for, above the line being typed.
        asks: &'static str,
        /// What has been typed of it, which may be nothing at all.
        typed: String,
    },
    /// Choosing the form a guard will stop.
    Picking {
        /// The guard being chosen for.
        lasting: &'static Lasting,
        /// The forms it can be given, one of them selected.
        chooser: Chooser<Choice>,
    },
    /// Holding the question before either of them starts.
    Beginning {
        /// The one about to start.
        lasting: &'static Lasting,
        /// What it was given.
        begun: Begun,
    },
    /// It is running, and being watched rather than waited for.
    Keeping {
        /// The one that is running.
        lasting: &'static Lasting,
        /// What it was given, in the words it will be spoken about by.
        named: String,
        /// Whether the screen offers to end it, which is the web's own answer for
        /// the same command rather than a second one.
        ends: bool,
        /// What it has said so far, where it says anything while it runs.
        said: Option<Reading>,
    },
    /// Choosing which change to make to the quality this stack aims for.
    Deciding(Chooser<&'static Change>),
    /// Choosing the preset one of them records.
    Grading {
        /// The change being chosen for.
        change: &'static Change,
        /// The presets, one of them selected.
        chooser: Chooser<Grade>,
    },
    /// Asking the core what a change would cost, before anybody agrees to it.
    Costing {
        /// The change being costed.
        change: &'static Change,
    },
    /// Holding the question, with the account above it where there is one to read.
    Settling {
        /// The change about to be made.
        change: &'static Change,
        /// The preset it was given, or nothing where it takes none.
        chosen: Option<&'static str>,
        /// What was read before the question, and where in it the box is.
        account: Option<Reading>,
    },
    /// The change is with the core.
    Applying {
        /// The change that is running.
        change: &'static Change,
        /// The preset it was given, or nothing where it takes none.
        chosen: Option<&'static str>,
    },
    /// Holding the question before the terminal is handed to the web surface.
    Handing,
}
