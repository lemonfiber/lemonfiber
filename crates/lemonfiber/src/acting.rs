//! What a keypress on the dashboard asks for, and what becomes of it.
//!
//! The dashboard read six panels and offered nothing else — no way to act on what
//! it read, and no way to ask anything the panels do not already show. This is the
//! deciding half of both: which key reaches which action, what an action may be
//! given, the question put before it, what this stack can be asked, and what is
//! said about every answer. None of it is in [`crate::terminal`], which is a real
//! terminal in raw mode and the one file this workspace deliberately does not test
//! — a decision behind that filename is a decision nothing checks.
//!
//! **An action reaches the command every other surface reaches, and so does a
//! question.** Both are named rather than assembled: an action goes through the
//! web surface's table of actions and a question through its table of reads, so
//! this screen cannot grow either a write or a read a browser has no form of. That
//! is the whole point of the arrangement: a terminal that did something no other
//! surface could do would defeat the requirement it was built for.
//!
//! **A read is asked for the same way an action is.** One key opens the list, the
//! entry taken names what to ask, and a question that has to be given a word gets a
//! line to type it on. The answer comes back in the words the command line gives
//! for the same request, in a box that moves through them — an answer cut to what
//! fits is an answer whose end nobody can reach.
//!
//! **Nothing happens on one keypress.** A key opens the list of what the action can
//! be given; taking one puts the question; only an explicit yes goes ahead. On a
//! screen where one finger reaches a teardown, the question is the difference
//! between an action and an accident — and it is where what is about to happen is
//! named, which the command line does with its own sentence before starting or
//! stopping.
//!
//! **What is about to happen is said before it is agreed to, in the words that are
//! true of it.** An errand that carries an agreement is run unconfirmed first, and
//! what it answers with is the box the question sits under; a quality change is not
//! all one shape, so [`quality`] puts a different thing there for each of the three
//! and says why on each. What none of them does is invent a preamble for symmetry.
//! An effect somebody reads after agreeing to it is not one they agreed to, and an
//! account of an effect that never happened is worse than none.
//!
//! **A long action reports through the screen it interrupted.** The web answers one
//! with a job's name because a request cannot be held open for minutes; a terminal
//! has no such indirection and needs none, because the dashboard is already the
//! report. The panels go on gathering every second while the work runs, so a
//! restart shows as the services going down and coming back, in the panel that
//! lists them. Nothing is drawn over that — only the footer says what is running.
//! The one exception is a walk, whose steps nothing behind the box is showing, and
//! which says them as they become true rather than at the end.
//!
//! **What has no ending of its own is offered one.** Every other thing this screen
//! sends finishes, so leaving is all it needs to offer. A guard does not: it holds
//! until the data location is lost, which on a machine where the drive stays put is
//! never. The web answers that with a lease it lets go of; a terminal answers it
//! with the interruption a shell would use, which on this screen is a keypress
//! rather than a signal. Which of them is which is asked of the web's own table in
//! [`lasting`] rather than decided a second time.
//!
//! **Leaving closes the screen, not the run.** A closed browser tab leaves a server
//! carrying the job on; a closed dashboard has no server to leave it to. The process
//! drawing this screen is the one that claimed the stack and issued the command, and
//! it is the only one that can give the stack back — so leaving gives the screen back
//! at once and the run stays until the action it started has finished. Saying
//! otherwise would leave an operator to find out from the next command, refused in
//! the name of a process that no longer exists.

mod answering;
mod bundling;
mod chooser;
mod disturbing;
mod errand;
mod inviting;
mod lasting;
mod mending;
mod narrowing;
mod offer;
mod quality;
mod question;
mod reading;
mod service;
mod stage;
mod surface;
mod words;

use lemonfiber_core::app::Command;
use lemonfiber_core::dashboard::Panel;
use lemonfiber_core::docker::Service;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Line;

use chooser::Chooser;
use lasting::Lasting;
use offer::Offer;
use stage::Stage;

/// The key that opens the list of what this stack can be asked.
///
/// Re-exported for the screen these boxes are drawn over, whose own tests press it
/// rather than writing the letter down a second time.
#[cfg(test)]
pub(crate) use question::KEY as ASK;
pub(crate) use words::Pane;

/// What the operator pressed.
pub(crate) enum Press {
    /// A character.
    Typed(char),
    /// Take back the last character typed.
    Rubout,
    /// The entry above the one selected.
    Back,
    /// The entry below it.
    Forward,
    /// Take what is selected.
    Accept,
    /// Back out of whatever is open.
    Abandon,
}

/// What a keypress on this screen is, or nothing for one it has no use for.
///
/// Beside the vocabulary it produces rather than in [`crate::terminal`], for the
/// reason everything else here is: which key reaches which action, and what backing
/// out of a half-answered question does, are decisions — and a decision behind that
/// filename is a decision nothing checks.
///
/// Ctrl-C, because a terminal in raw mode no longer turns it into a signal and an
/// operator who cannot back out with it is trapped.
pub(crate) const fn meaning(key: KeyEvent) -> Option<Press> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Some(Press::Abandon),
            _ => None,
        };
    }
    match key.code {
        KeyCode::Char(character) => Some(Press::Typed(character)),
        KeyCode::Backspace => Some(Press::Rubout),
        KeyCode::Esc => Some(Press::Abandon),
        KeyCode::Enter => Some(Press::Accept),
        KeyCode::Up => Some(Press::Back),
        KeyCode::Down => Some(Press::Forward),
        _ => None,
    }
}

/// Whether a press is the operator leaving the screen.
///
/// One predicate rather than the same pair of keys written out beside each thing
/// that can be running. Three flows have something the operator can leave while it
/// is with the core, and the day either key changes is the day the two that kept
/// their own copy stop agreeing with the one that did not about what leaving is.
const fn leaving(press: &Press) -> bool {
    matches!(*press, Press::Typed('q') | Press::Abandon)
}

/// What the loop has to go and do about a press.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Wanted {
    /// Nothing the loop has to go and do.
    Nothing,
    /// Ask the core this, and hand the answer back through [`Acting::told`].
    Ask(Command),
    /// Carry this out, and hand what it came to back through [`Acting::came_to`].
    Carry(Command),
    /// Record what the pane of words has just explained.
    Words,
    /// Gather afresh now rather than at the next tick.
    Gather,
    /// End what is running, which is what an interruption does at a terminal.
    ///
    /// Only ever asked for the one command with no ending of its own. Everything
    /// else this screen sends finishes, and ending work that was going to succeed
    /// because somebody pressed escape is not something to offer.
    Stop,
    /// Leave the dashboard, and start the web surface on the terminal it gives back,
    /// with the three choices the screen was asked for before it closed.
    Serve(crate::ui::Asked),
    /// Leave the dashboard.
    Leave,
}

/// What an outstanding [`Wanted::Ask`] was begun for.
///
/// Taken when the answer arrives, so an answer to a question that was never asked —
/// or was asked for something else — changes nothing. Two things are chosen a subject
/// off the stack's own list of forms and they are chosen it the same way, so the
/// answer has to say which of them was waiting for it.
enum Asked {
    /// One of the five actions on a key of its own.
    Action(&'static Offer),
    /// The guard, which insists on a form the way three of those five do.
    Guard(&'static Lasting),
}

/// What this screen has open, and what it is waiting for.
pub(crate) struct Acting {
    /// Where the action stands.
    stage: Stage,
    /// What an outstanding [`Wanted::Ask`] was begun for.
    asked: Option<Asked>,
    /// Whether the pane explaining this screen's words is open.
    words: bool,
    /// Whether this run explains its words at all.
    ///
    /// Given at the edge, where whether a run explains anything is known, rather
    /// than read here — the same division the log viewer's own state makes.
    explanations: bool,
    /// The services the panels are showing, for the lists that name one.
    ///
    /// Kept from the gather rather than asked for when a key is pressed. A read is
    /// awaited in the loop with no frame drawn between the asking and the answer,
    /// which is why the one read this screen makes that way is a file on disk — and
    /// what each service is doing reaches the container engine.
    services: Vec<(String, String, String)>,
}

impl Acting {
    /// A screen with nothing open.
    pub(crate) const fn opened() -> Self {
        Self {
            stage: Stage::Idle,
            asked: None,
            words: false,
            explanations: true,
            services: Vec::new(),
        }
    }

    /// Take the services out of a gather that has just landed.
    ///
    /// Every gather rather than only the ones a list is open over: which services
    /// there are is a fact about the stack, and a screen holding the ones it saw when
    /// a key was first pressed would offer a service that has since been taken out of
    /// the manifest.
    pub(crate) fn gathered(&mut self, services: &Panel<Vec<Service>>) {
        self.services = service::gathered(services);
    }

    /// The same, on a run that does not explain its words.
    #[must_use]
    pub(crate) fn without_explanations(self) -> Self {
        Self {
            explanations: false,
            ..self
        }
    }

    /// Whether the pane explaining this screen's words is open.
    pub(crate) const fn showing_words(&self) -> bool {
        self.words
    }

    /// What a press asks for.
    /// What a press asks for.
    ///
    /// Routed in two, because the stages a press can arrive at are two kinds. One is
    /// a flow that began by taking something off a list of things to *do* — an
    /// errand, one of the two that keep going, a quality change, or a repair. The
    /// other is a flow that began at a key: the five lifecycle actions and the
    /// services inside what they named, the questions, the readings a press moves
    /// through, and the hand-over, which a key reaches and no list holds. Each half claims its own and hands back what is not, so
    /// what neither claims is the screen with nothing open — where a press belongs to
    /// no flow at all and is the screen's own.
    pub(crate) fn pressed(&mut self, press: &Press) -> Wanted {
        // The pane of words closes on any key and takes the key with it, which is
        // what makes it dismissible without anybody having to learn how.
        if self.words {
            self.words = false;
            return Wanted::Nothing;
        }
        match self.off_a_list(press) {
            Some(wanted) => wanted,
            None => match self.on_a_key(press) {
                Some(wanted) => wanted,
                // Only one stage is neither, and it is the one with nothing open.
                None => self.idle(press),
            },
        }
    }

    /// A press over a flow that began by taking something off a list of things to do,
    /// or nothing where it began somewhere else — the stage put back as it was.
    fn off_a_list(&mut self, press: &Press) -> Option<Wanted> {
        Some(match std::mem::replace(&mut self.stage, Stage::Idle) {
            Stage::Sending(chooser) => {
                errand::sending(&mut self.stage, chooser, press, &self.services)
            }
            Stage::Naming {
                errand,
                asks,
                typed,
            } => errand::naming(&mut self.stage, errand, asks, typed, press),
            Stage::Bundling { errand, chooser } => {
                bundling::bundling(&mut self.stage, errand, chooser, press)
            }
            Stage::Allowing {
                errand,
                name,
                typed,
            } => inviting::allowing(&mut self.stage, errand, name, typed, press),
            Stage::Limiting { errand, chooser } => {
                inviting::limited(&mut self.stage, errand, chooser, press)
            }
            Stage::Unrated { errand, chooser } => {
                inviting::unrated(&mut self.stage, errand, chooser, press)
            }
            Stage::Weighing { errand, given } => {
                errand::weighing(&mut self.stage, errand, given, press)
            }
            Stage::Agreeing {
                errand,
                given,
                would,
            } => errand::agreeing(&mut self.stage, errand, given, would, press),
            Stage::Doing { errand, given } => errand::doing(&mut self.stage, errand, given, press),
            Stage::Starting(chooser) => {
                lasting::starting(&mut self.stage, &mut self.asked, chooser, press)
            }
            Stage::Wording {
                lasting,
                asks,
                typed,
            } => lasting::wording(&mut self.stage, lasting, asks, typed, press),
            Stage::Picking { lasting, chooser } => {
                lasting::picking(&mut self.stage, lasting, chooser, press)
            }
            Stage::Beginning { lasting, begun } => {
                lasting::beginning(&mut self.stage, lasting, begun, press)
            }
            Stage::Keeping {
                lasting,
                named,
                ends,
                said,
            } => lasting::keeping(&mut self.stage, lasting, named, ends, said, press),
            Stage::Deciding(chooser) => quality::deciding(&mut self.stage, chooser, press),
            Stage::Scoping { change, chooser } => {
                quality::scoping(&mut self.stage, change, chooser, press)
            }
            Stage::Grading {
                change,
                chosen,
                chooser,
            } => quality::grading(&mut self.stage, change, chosen, chooser, press),
            Stage::Costing { change } => quality::costing(&mut self.stage, change, press),
            Stage::Settling {
                change,
                chosen,
                account,
            } => quality::settling(&mut self.stage, change, chosen, account, press),
            Stage::Applying { change, chosen } => {
                quality::applying(&mut self.stage, change, chosen, press)
            }
            Stage::Righting(chooser) => mending::righting(&mut self.stage, chooser, press),
            Stage::Looking(mending) => mending::looking(&mut self.stage, mending, press),
            Stage::Marking { mending, offering } => {
                mending::marking(&mut self.stage, mending, offering, press)
            }
            Stage::Consenting { mending, agreed } => {
                mending::consenting(&mut self.stage, mending, agreed, press)
            }
            Stage::Warned { mending, chooser } => {
                mending::warned(&mut self.stage, mending, chooser, press)
            }
            Stage::Answering { mending, warning } => {
                mending::answering(&mut self.stage, mending, warning, press)
            }
            Stage::Putting(mending) => mending::putting(&mut self.stage, mending, press),
            // A reading moves, and any key that is not a move puts it away — the
            // way the pane of words is put away.
            // Put back rather than carried out: a stage this does not claim is one
            // the dispatcher below it does, and it has to be there to be taken again.
            elsewhere => {
                self.stage = elsewhere;
                return None;
            }
        })
    }

    /// A press over a flow that began at a key, or nothing where none is open — the
    /// stage put back as it was.
    fn on_a_key(&mut self, press: &Press) -> Option<Wanted> {
        Some(match std::mem::replace(&mut self.stage, Stage::Idle) {
            Stage::Choosing { offer, chooser } => {
                offer::choosing(&mut self.stage, offer, chooser, press, &self.services)
            }
            Stage::Inside { inside, chooser } => {
                service::choosing(&mut self.stage, inside, chooser, press)
            }
            Stage::Confirming { offer, taken } => {
                offer::confirming(&mut self.stage, offer, taken, press)
            }
            Stage::Running { offer, taken } => offer::running(&mut self.stage, offer, taken, press),
            Stage::Wondering(chooser) => question::wondering(&mut self.stage, chooser, press),
            Stage::Typing {
                question,
                said,
                typed,
            } => question::typing(&mut self.stage, question, said, typed, press),
            Stage::Waiting { question, said } => {
                question::waiting(&mut self.stage, Stage::Waiting { question, said }, press)
            }
            Stage::Following(question) => {
                question::waiting(&mut self.stage, Stage::Following(question), press)
            }
            Stage::Narrowing { question, chooser } => {
                narrowing::narrowing(&mut self.stage, question, chooser, press)
            }
            Stage::Came(reading) => reading::came(&mut self.stage, reading, press),
            Stage::Answered {
                question,
                widening,
                reading,
            } => question::answered(&mut self.stage, question, widening, reading, press),
            Stage::Disturbing(widened) => disturbing::disturbing(&mut self.stage, widened, press),
            Stage::Handing { asked, open } => surface::handing(&mut self.stage, asked, open, press),
            // Put back rather than carried out: a stage this does not claim is one
            // the dispatcher below it does, and it has to be there to be taken again.
            elsewhere => {
                self.stage = elsewhere;
                return None;
            }
        })
    }

    /// With nothing open: leave, gather afresh, explain, or begin an action.
    fn idle(&mut self, press: &Press) -> Wanted {
        match *press {
            Press::Typed('q') | Press::Abandon => Wanted::Leave,
            Press::Typed('r') => Wanted::Gather,
            Press::Typed('?') if self.explanations => {
                self.words = true;
                Wanted::Words
            }
            Press::Typed(question::KEY) => {
                let (first, rest) = question::all();
                self.stage = Stage::Wondering(Chooser::over(first, rest));
                Wanted::Nothing
            }
            Press::Typed(errand::KEY) => {
                let (first, rest) = errand::all();
                self.stage = Stage::Sending(Chooser::over(first, rest));
                Wanted::Nothing
            }
            Press::Typed(lasting::KEY) => {
                let (first, rest) = lasting::all();
                self.stage = Stage::Starting(Chooser::over(first, rest));
                Wanted::Nothing
            }
            Press::Typed(quality::KEY) => {
                let (first, rest) = quality::all();
                self.stage = Stage::Deciding(Chooser::over(first, rest));
                Wanted::Nothing
            }
            Press::Typed(mending::KEY) => {
                let (first, rest) = mending::all();
                self.stage = Stage::Righting(Chooser::over(first, rest));
                Wanted::Nothing
            }
            Press::Typed(surface::KEY) => {
                self.stage = surface::asking();
                Wanted::Nothing
            }
            Press::Typed(key) => offer::begin(&mut self.asked, key),
            Press::Rubout | Press::Back | Press::Forward | Press::Accept => Wanted::Nothing,
        }
    }

    /// The box over the screen, or nothing where the action has none open.
    pub(crate) fn pane(&self, rows: usize, across: usize) -> Option<Pane> {
        words::pane(&self.stage, rows, across)
    }

    /// The one line at the foot of the screen.
    pub(crate) fn footer(&self, across: usize) -> Line<'static> {
        words::footer(&self.stage, across)
    }

    /// What a run leaving this screen now would stay for, or nothing where it may go.
    ///
    /// Asked for on the way out, where the loop knows the screen is being given back
    /// and this knows what is still with the core.
    pub(crate) fn staying_for(&self) -> Option<String> {
        words::staying_for(&self.stage)
    }
}

#[cfg(test)]
mod tests;
