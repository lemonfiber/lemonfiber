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
//!
//! The seventh key opens what can be done about what a diagnosis found, and its two
//! flows are the same shape either way: something is read first, one of the things it
//! held is chosen, and only then is the question put. What differs is what is read and
//! how many of it can be taken — [`super::mending`] holds both.
//!
//! One stage is reached by no key at all. The checks that disturb a running system
//! are offered under the diagnosis that does not, so what opens that question is an
//! answer already on the screen — which is why [`Stage::Answered`] carries the offer
//! and [`super::disturbing`] decides what becomes of it.

use super::bundling::Held;
use super::chooser::Chooser;
use super::disturbing::Widening;
use super::errand::{Errand, Given};
use super::inviting::Limit;
use super::lasting::{Begun, Lasting};
use super::mending::{Agreed, Mending, Offering, Warning};
use super::narrowing::Subject;
use super::offer::{Choice, Offer, Taken};
use super::quality::{Change, Chosen, Grade, Scope};
use super::question::Question;
use super::reading::Reading;
use super::service::Inside;
use super::surface::Open;
use crate::ui::Asked;

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
    /// Choosing which services inside what was named to act on.
    ///
    /// One stage for both flows that reach it, because they reach the same list: what
    /// each of them goes on to is [`super::service::Inside`]'s, which is the whole of
    /// what the two have that is not shared.
    Inside {
        /// What the services are being named for, and what naming them leads to.
        inside: Inside,
        /// The services, some of them marked.
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
    /// Typing the words a question has to be given, one line at a time.
    ///
    /// What is asked for above the line is not held here: it is read off how many
    /// words are already in hand, so the prompt and the argument the next word fills
    /// cannot come apart.
    Typing {
        /// The question waiting on them.
        question: &'static Question,
        /// The words already given, in the order they were asked for.
        said: Vec<String>,
        /// What has been typed of the next one.
        typed: String,
    },
    /// The question is with the core.
    ///
    /// The words it was given travel with it, because what one of them narrowed is
    /// what a widening offered under the answer is narrowed by — and by the time the
    /// answer arrives the line they were typed on is gone.
    Waiting {
        /// The question waiting on the core.
        question: &'static Question,
        /// The words it was given, empty where it takes none.
        said: Vec<String>,
    },
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
        /// The widened run offered under it, where the answer is a diagnosis.
        widening: Option<Widening>,
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
    /// Choosing what a bundle does with media filenames, the window having been typed.
    Bundling {
        /// The errand being chosen for, which is the one that writes a bundle.
        errand: &'static Errand,
        /// The two answers, one of them selected, each carrying the whole of what the
        /// bundle would then be asked for.
        chooser: Chooser<Held>,
    },
    /// Typing which libraries an invitation lets somebody open, the name having been
    /// typed.
    Allowing {
        /// The errand waiting on it, which is the one that offers an account.
        errand: &'static Errand,
        /// Who it is for, kept because the question after this one says all three
        /// answers together and the line they were typed on is gone by then.
        name: String,
        /// What has been typed of the libraries.
        typed: String,
    },
    /// Choosing how far up the ratings an invitation goes, the rest having been typed.
    Limiting {
        /// The errand being chosen for, which is the one that offers an account.
        errand: &'static Errand,
        /// No limit and each step above it, one of them selected, each carrying the
        /// whole of what the invitation would then be asked for.
        chooser: Chooser<Limit>,
    },
    /// Asking the core what the errand would do, before anybody agrees to it.
    Weighing {
        /// The errand being weighed.
        errand: &'static Errand,
        /// What it was given, which is nothing at all where it takes nothing.
        given: Given,
    },
    /// Holding the question, with what it would do above it where the errand can say.
    Agreeing {
        /// The errand about to be sent.
        errand: &'static Errand,
        /// What it was given, which is nothing at all where it takes nothing.
        given: Given,
        /// What it would do, and where in it the box is, where it said.
        would: Option<Reading>,
    },
    /// The errand is with the core.
    Doing {
        /// The errand that is running.
        errand: &'static Errand,
        /// What it was given, which is nothing at all where it takes nothing.
        given: Given,
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
    /// Choosing which media the change is about.
    ///
    /// In front of the bars rather than after them, because what a bar even is
    /// depends on the answer: music has no resolution, so choosing for it picks an
    /// audio format and the translation reaches a different command with it.
    Scoping {
        /// The change being chosen for.
        change: &'static Change,
        /// The media it can be about, one of them selected.
        chooser: Chooser<Scope>,
    },
    /// Choosing the bar one of them records for that media.
    Grading {
        /// The change being chosen for.
        change: &'static Change,
        /// The media it is about, taken off the list before this one.
        chosen: Chosen,
        /// The bars, one of them selected.
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
        /// The media and the bar it was given, empty where it takes neither.
        chosen: Chosen,
        /// What was read before the question, and where in it the box is.
        account: Option<Reading>,
    },
    /// The change is with the core.
    Applying {
        /// The change that is running.
        change: &'static Change,
        /// The media and the bar it was given, empty where it takes neither.
        chosen: Chosen,
    },
    /// Choosing what to do about what a diagnosis found.
    Righting(Chooser<&'static Mending>),
    /// What has to be read before that question can be put is with the core.
    ///
    /// One stage for both, because both are the same thing to be waiting for: what
    /// the operator has to have read before there is anything to agree to. What each
    /// of them asked for is the entry's own, and what comes back is told apart by it.
    Looking(&'static Mending),
    /// Marking the repairs to agree to, out of the offer they were read in.
    Marking {
        /// The write being agreed to, which is what the box is titled by.
        mending: &'static Mending,
        /// The offer, as it named itself and as it stands on the screen.
        offering: Offering,
    },
    /// Holding the question over the repairs marked, under what each would do.
    ///
    /// The name of the offer travels this far and no further: it goes into the
    /// request the yes sends, so what was agreed to cannot be spent on an offer that
    /// has moved on since it was read.
    Consenting {
        /// The write being agreed to.
        mending: &'static Mending,
        /// What was agreed to, and the words it was agreed to in.
        agreed: Agreed,
    },
    /// Choosing which of the warnings this stack raises to answer.
    Warned {
        /// The write being chosen for.
        mending: &'static Mending,
        /// The warnings it raised, one of them selected.
        chooser: Chooser<Warning>,
    },
    /// Holding the question before a warning is answered.
    Answering {
        /// The write being agreed to.
        mending: &'static Mending,
        /// The warning it is about to answer.
        warning: Warning,
    },
    /// The putting-right is with the core.
    Putting(&'static Mending),
    /// The checks that disturb a running system are with the core.
    ///
    /// It carries nothing, because there is nothing left to decide: what it was
    /// narrowed to went into the command when the offer was agreed to, and the panels
    /// behind this are the report.
    Disturbing,
    /// Holding the question before the terminal is handed to the web surface, and
    /// whatever is open under it.
    Handing {
        /// What the surface is about to be started with.
        asked: Asked,
        /// The three choices being read, chosen between, or typed at.
        open: Open,
    },
}
