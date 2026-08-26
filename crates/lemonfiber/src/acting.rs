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

mod chooser;
mod errand;
mod lasting;
mod offer;
mod quality;
mod question;
mod reading;
mod stage;
mod surface;
mod words;

use lemonfiber_core::app::{Command, Outcome};
use lemonfiber_core::error::Problem;
use lemonfiber_core::walkthrough::Line as Step;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Line;

use chooser::Chooser;
use lasting::Lasting;
use offer::Offer;
use reading::{complaint, lines_of, moved, unexpected, Reading};
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
    /// Leave the dashboard, and start the web surface on the terminal it gives back.
    Serve,
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
}

impl Acting {
    /// A screen with nothing open.
    pub(crate) const fn opened() -> Self {
        Self {
            stage: Stage::Idle,
            asked: None,
            words: false,
            explanations: true,
        }
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
    pub(crate) fn pressed(&mut self, press: &Press) -> Wanted {
        // The pane of words closes on any key and takes the key with it, which is
        // what makes it dismissible without anybody having to learn how.
        if self.words {
            self.words = false;
            return Wanted::Nothing;
        }
        match std::mem::replace(&mut self.stage, Stage::Idle) {
            Stage::Idle => self.idle(press),
            Stage::Choosing { offer, chooser } => {
                offer::choosing(&mut self.stage, offer, chooser, press)
            }
            Stage::Confirming { offer, taken } => {
                offer::confirming(&mut self.stage, offer, taken, press)
            }
            Stage::Running { offer, taken } => offer::running(&mut self.stage, offer, taken, press),
            Stage::Wondering(chooser) => question::wondering(&mut self.stage, chooser, press),
            Stage::Typing {
                question,
                asks,
                typed,
            } => question::typing(&mut self.stage, question, asks, typed, press),
            Stage::Waiting(question) => question::waiting(&mut self.stage, question, press),
            Stage::Sending(chooser) => errand::sending(&mut self.stage, chooser, press),
            Stage::Naming {
                errand,
                asks,
                typed,
            } => errand::naming(&mut self.stage, errand, asks, typed, press),
            Stage::Weighing { errand, typed } => {
                errand::weighing(&mut self.stage, errand, typed, press)
            }
            Stage::Agreeing {
                errand,
                typed,
                would,
            } => errand::agreeing(&mut self.stage, errand, typed, would, press),
            Stage::Doing { errand, typed } => errand::doing(&mut self.stage, errand, typed, press),
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
            Stage::Grading { change, chooser } => {
                quality::grading(&mut self.stage, change, chooser, press)
            }
            Stage::Costing { change } => quality::costing(&mut self.stage, change, press),
            Stage::Settling {
                change,
                chosen,
                account,
            } => quality::settling(&mut self.stage, change, chosen, account, press),
            Stage::Applying { change, chosen } => {
                quality::applying(&mut self.stage, change, chosen, press)
            }
            Stage::Handing => surface::handing(&mut self.stage, press),
            // A reading moves, and any key that is not a move puts it away — the
            // way the pane of words is put away.
            Stage::Came(mut reading) => {
                if moved(&mut reading, press) {
                    self.stage = Stage::Came(reading);
                }
                Wanted::Nothing
            }
            Stage::Answered {
                question,
                mut reading,
            } => {
                if moved(&mut reading, press) {
                    self.stage = Stage::Answered { question, reading };
                }
                Wanted::Nothing
            }
        }
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
            Press::Typed(surface::KEY) => {
                self.stage = Stage::Handing;
                Wanted::Nothing
            }
            Press::Typed(key) => offer::begin(&mut self.asked, key),
            Press::Rubout | Press::Back | Press::Forward | Press::Accept => Wanted::Nothing,
        }
    }

    /// What the stack answered when it was asked what there is to act on.
    ///
    /// One answer and two things that may have been waiting for it: an action, and
    /// the guard, which insists on a form the same way three of the five actions do.
    /// Which of them was waiting is what the screen recorded when it asked.
    pub(crate) fn told(&mut self, answer: Result<Outcome, Box<Problem>>) {
        let Some(asked) = self.asked.take() else {
            return;
        };
        let report = match answer {
            Ok(Outcome::Forms(report)) => report,
            Ok(_) => {
                self.stage = Stage::Came(Reading::of(unexpected()));
                return;
            }
            Err(problem) => {
                self.stage = Stage::Came(Reading::of(complaint(&problem)));
                return;
            }
        };
        self.stage = match asked {
            Asked::Action(offer) => match offer.given(&report) {
                Ok((selected, rest)) => Stage::Choosing {
                    offer,
                    chooser: Chooser::over(selected, rest),
                },
                Err(refused) => Stage::Came(Reading::of(vec![refused])),
            },
            Asked::Guard(lasting) => match offer::choices(lasting.action, &report) {
                Ok((selected, rest)) => Stage::Picking {
                    lasting,
                    chooser: Chooser::over(selected, rest),
                },
                Err(refused) => Stage::Came(Reading::of(vec![refused])),
            },
        };
    }

    /// One of a walk's steps, as the core said it.
    ///
    /// Handed on rather than rendered here: what a step reads as on a terminal is
    /// [`crate::render::walkthrough`]'s, and it is the same rendering a shell is
    /// given for the same walk.
    pub(crate) fn stepped(&mut self, line: &Step) {
        lasting::stepped(&mut self.stage, line);
    }

    /// What the action came to, or what the question was answered with.
    ///
    /// Which of the two is read off what the screen is waiting for. An answer
    /// nobody is waiting for any more — a question backed out of while it was with
    /// the core — leaves the screen as it stands rather than opening a box over
    /// whatever the operator went on to do.
    pub(crate) fn came_to(&mut self, answer: Result<Outcome, Box<Problem>>) {
        self.stage = match std::mem::replace(&mut self.stage, Stage::Idle) {
            Stage::Running { .. } => Stage::Came(Reading::of(match answer {
                Ok(Outcome::Lifecycle(report)) => {
                    lines_of(&crate::render::stack::lifecycle(&report))
                }
                Ok(_) => unexpected(),
                Err(problem) => complaint(&problem),
            })),
            // What an errand would do is the answer the operator reads before
            // agreeing, so it lands on the question rather than closing over it. A
            // failure ends the errand there: there is nothing to agree to.
            Stage::Weighing { errand, typed } => match answer {
                Ok(outcome) => {
                    errand::weighed(errand, typed, lines_of(&crate::render::shaped(&outcome)))
                }
                Err(problem) => Stage::Came(Reading::of(complaint(&problem))),
            },
            Stage::Doing { .. } => Stage::Came(Reading::of(match answer {
                Ok(outcome) => lines_of(&crate::render::shaped(&outcome)),
                Err(problem) => complaint(&problem),
            })),
            // What a walk came to goes under the steps that were watched on the way,
            // which is the order a shell shows them in. A guard has said nothing
            // until now, so what it came to is the whole of its box.
            Stage::Keeping { said, .. } => lasting::came_to(
                said,
                match answer {
                    Ok(outcome) => lines_of(&crate::render::shaped(&outcome)),
                    Err(problem) => complaint(&problem),
                },
            ),
            // What an upgrade would cost is read before it is agreed to, so it lands
            // on the question the way an errand's account does.
            Stage::Costing { change } => match answer {
                Ok(outcome) => quality::costed(change, lines_of(&crate::render::shaped(&outcome))),
                Err(problem) => Stage::Came(Reading::of(complaint(&problem))),
            },
            // A choice this host could only transcode in software comes back held
            // rather than recorded, and the caution it comes back with is the account
            // the question that follows sits under.
            Stage::Applying { change, chosen } => match answer {
                Ok(outcome) => quality::applied(
                    change,
                    chosen,
                    &outcome,
                    lines_of(&crate::render::shaped(&outcome)),
                ),
                Err(problem) => Stage::Came(Reading::of(complaint(&problem))),
            },
            // Rendered by the same renderer the command line reaches for the same
            // answer, so the two surfaces cannot come to say different things about
            // one stack.
            Stage::Waiting(question) => Stage::Answered {
                question,
                reading: Reading::of(match answer {
                    Ok(outcome) => lines_of(&crate::render::shaped(&outcome)),
                    Err(problem) => complaint(&problem),
                }),
            },
            waiting_for_nothing => waiting_for_nothing,
        };
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
mod tests {
    use super::chooser::Chooser;
    use super::offer::tests::{a_listing, nothing_declared};
    use super::{
        errand, lasting, meaning, quality, question, surface, Acting, Line, Press, Step, Wanted,
    };
    use crate::render::fixtures::{a_lifecycle, a_plan};
    use lemonfiber_core::app::restore::Kept;
    use lemonfiber_core::app::{backup, Command, Outcome, QualityAction, Waiting};
    use lemonfiber_core::backup::Scope;
    use lemonfiber_core::error::{Code, Problem, Remedy, Severity};
    use lemonfiber_core::model::{
        Disposition, FormsReport, PresetChoice, QualityReport, ResetReport, StackEdit,
        SupervisionReport, UpgradeMedia, UpgradeReport, VersionReport,
    };
    use lemonfiber_core::quality::Preset;
    use lemonfiber_core::walkthrough::Step as WalkStep;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    /// The screen, having got as far as the list for one action.
    fn choosing(key: char, report: FormsReport) -> (Acting, Wanted) {
        let mut acting = Acting::opened();
        let wanted = acting.pressed(&Press::Typed(key));
        acting.told(Ok(Outcome::Forms(report)));
        (acting, wanted)
    }

    /// Everything the pane says, as one piece of text.
    fn showing(acting: &Acting) -> String {
        acting.pane(20, 100).map_or_else(String::new, |pane| {
            let mut said = vec![pane.title.clone()];
            said.extend(pane.lines.iter().map(text));
            said.join("\n")
        })
    }

    /// One line as text, its spans joined the way the screen shows them.
    fn text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<Vec<&str>>()
            .concat()
    }

    /// The footer, as text.
    fn footing(acting: &Acting) -> String {
        text(&acting.footer(200))
    }

    /// A version report, which is both an answer this screen asks for and an answer
    /// of a shape an action never has.
    fn a_version() -> VersionReport {
        VersionReport {
            binary: "0.8.0".to_owned(),
            supported_schema: vec![1],
            stack: "1.2.3".to_owned(),
            compose: None,
        }
    }

    /// The screen, having got as far as the line where what to follow is typed.
    fn asking_where() -> Acting {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(question::KEY));
        while !showing(&acting).contains("> where one thing is") {
            acting.pressed(&Press::Forward);
        }
        acting.pressed(&Press::Accept);
        acting
    }

    /// A failure to render, for the paths that report one.
    fn a_failure() -> Problem {
        Problem::new(
            Code::new("TEST-1"),
            Severity::Error,
            "the container engine could not be reached",
            "Nothing can be started or stopped until it answers.",
            Remedy::new("Start the container engine"),
        )
    }

    /// The whole flow, which is the claim this screen exists to make: a key, a
    /// choice, a question, an explicit yes, and only then a command — and the
    /// command is one of the core's own.
    #[test]
    fn an_action_takes_a_key_a_choice_and_an_answer_before_it_reaches_a_command() {
        let (mut acting, first) = choosing('t', a_listing());
        assert_eq!(first, Wanted::Ask(Command::Forms));

        assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
        let carried = acting.pressed(&Press::Typed('y'));

        assert_eq!(
            carried,
            Wanted::Carry(Command::Restart {
                forms: vec!["full".to_owned()],
                services: Vec::new(),
            })
        );
    }

    /// Moving back up the list lands on what it started at, and a stray character
    /// pressed over a list is not an answer to it — a screen where any key took the
    /// selected entry would act on a keypress meant for something else.
    #[test]
    fn moving_over_the_list_and_typing_at_it_take_nothing() {
        let (mut acting, _) = choosing('t', a_listing());

        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Back);
        assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);
        let said = showing(&acting);
        assert!(said.contains("> [ ] Full stack"), "{said}");

        acting.pressed(&Press::Accept);
        let carried = acting.pressed(&Press::Typed('y'));

        assert_eq!(
            carried,
            Wanted::Carry(Command::Restart {
                forms: vec!["full".to_owned()],
                services: Vec::new(),
            })
        );
    }

    /// What a lifecycle command came to is shown in the words the command line
    /// gives for the same run, rather than in a second account of it.
    #[test]
    fn a_lifecycle_report_is_shown_in_the_words_the_command_line_gives() {
        let report = a_lifecycle("restart", a_plan("full", Vec::new()));
        let printed = crate::render::stack::lifecycle(&report).text();
        let (mut acting, _) = choosing('t', a_listing());
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Typed('y'));

        acting.came_to(Ok(Outcome::Lifecycle(report)));

        let said = showing(&acting);
        for line in printed.lines().filter(|line| !line.is_empty()) {
            assert!(said.contains(line), "{line:?} is missing from {said}");
        }
    }

    /// The claim this slice exists to make: the list names several forms at once,
    /// which is what the command line takes and what a browser sends whole, and the
    /// several reach one command over all of them rather than one command each.
    #[test]
    fn marking_several_forms_acts_on_every_one_of_them() {
        let (mut acting, _) = choosing('t', a_listing());

        acting.pressed(&Press::Typed(' '));
        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Typed(' '));
        let listed = showing(&acting);
        assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
        let asked = showing(&acting);
        let carried = acting.pressed(&Press::Typed('y'));

        assert!(listed.contains("[x] Full stack"), "{listed}");
        assert!(listed.contains("[x] Lean stack"), "{listed}");
        assert!(listed.contains("enter takes the 2 marked"), "{listed}");
        assert!(asked.contains("Restart 2 forms?"), "{asked}");
        assert!(asked.contains("Full stack"), "{asked}");
        assert!(asked.contains("Lean stack"), "{asked}");
        assert_eq!(
            carried,
            Wanted::Carry(Command::Restart {
                forms: vec!["full".to_owned(), "lean".to_owned()],
                services: Vec::new(),
            })
        );
    }

    /// A mark comes off the way it went on, and a list with every mark taken off is
    /// the list it was: enter falls back to the row under the cursor. Nobody should
    /// have to abandon a list to escape a keypress they did not mean.
    #[test]
    fn taking_every_mark_off_again_leaves_the_row_under_the_cursor() {
        let (mut acting, _) = choosing('t', a_listing());

        acting.pressed(&Press::Typed(' '));
        acting.pressed(&Press::Typed(' '));
        let listed = showing(&acting);
        acting.pressed(&Press::Accept);
        let carried = acting.pressed(&Press::Typed('y'));

        assert!(listed.contains("enter takes this one"), "{listed}");
        assert_eq!(
            carried,
            Wanted::Carry(Command::Restart {
                forms: vec!["full".to_owned()],
                services: Vec::new(),
            })
        );
    }

    /// The whole stack is instead of naming forms rather than one more of them, so
    /// marking it takes the marks off the forms and what is sent is the empty list
    /// the command reads as everything — never that list with a form's name also in
    /// it, which would be a request for something other than what was shown.
    #[test]
    fn marking_the_whole_stack_takes_the_marks_off_the_forms() {
        let (mut acting, _) = choosing('u', a_listing());

        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Typed(' '));
        acting.pressed(&Press::Back);
        acting.pressed(&Press::Typed(' '));
        let listed = showing(&acting);
        acting.pressed(&Press::Accept);
        let carried = acting.pressed(&Press::Typed('y'));

        assert!(listed.contains("[x] everything"), "{listed}");
        assert!(listed.contains("[ ] Full stack"), "{listed}");
        assert_eq!(carried, Wanted::Carry(Command::Up { forms: Vec::new() }));
    }

    /// A guard is chosen off the same list by the same movement, so it names several
    /// forms at once too. One list behaving two ways depending on which key opened it
    /// is the thing the shared movement exists to prevent.
    #[test]
    fn a_guard_can_be_set_over_several_forms_at_once() {
        let mut acting = starting("watch");
        acting.told(Ok(Outcome::Forms(a_listing())));

        acting.pressed(&Press::Typed(' '));
        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Typed(' '));
        acting.pressed(&Press::Accept);
        let asked = showing(&acting);
        let carried = acting.pressed(&Press::Typed('y'));

        assert!(asked.contains("2 forms?"), "{asked}");
        assert_eq!(
            carried,
            Wanted::Carry(Command::Watch {
                forms: vec!["full".to_owned(), "lean".to_owned()],
            })
        );
    }

    /// While several are running the footer says how many rather than running their
    /// names together, and the line on the way out says the same — one row, on a
    /// screen whose width belongs to the panels behind it.
    #[test]
    fn a_running_action_over_several_says_how_many_it_is_running_on() {
        let (mut acting, _) = choosing('t', a_listing());
        acting.pressed(&Press::Typed(' '));
        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Typed(' '));
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Typed('y'));

        let footer = footing(&acting);
        let leaving = acting.staying_for().unwrap_or_default();

        assert!(footer.contains("restart 2 forms"), "{footer}");
        assert!(leaving.contains("restart 2 forms"), "{leaving}");
    }

    /// Choosing something further down the list acts on that one, which is the
    /// difference between a list and a decoration.
    #[test]
    fn what_was_selected_is_what_is_acted_on() {
        let (mut acting, _) = choosing('t', a_listing());

        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Accept);
        let carried = acting.pressed(&Press::Typed('Y'));

        assert_eq!(
            carried,
            Wanted::Carry(Command::Restart {
                forms: vec!["lean".to_owned()],
                services: Vec::new(),
            })
        );
    }

    /// The two actions whose command can carry an empty list offer the whole stack,
    /// and taking it names no form at all.
    #[test]
    fn the_whole_stack_is_a_choice_where_the_command_can_mean_it() {
        let (mut acting, _) = choosing('d', a_listing());

        acting.pressed(&Press::Accept);
        let carried = acting.pressed(&Press::Typed('y'));

        assert_eq!(
            carried,
            Wanted::Carry(Command::Down {
                forms: Vec::new(),
                wait: Waiting::Never
            })
        );
    }

    /// Anything but an explicit yes changes nothing, and leaves the screen where an
    /// operator can start again.
    #[test]
    fn a_question_answered_with_anything_else_changes_nothing() {
        for answer in [
            Press::Typed('n'),
            Press::Typed('\n'),
            Press::Accept,
            Press::Abandon,
            Press::Forward,
        ] {
            let (mut acting, _) = choosing('d', a_listing());
            acting.pressed(&Press::Accept);

            let answered = acting.pressed(&answer);

            assert_eq!(answered, Wanted::Nothing);
            assert!(showing(&acting).is_empty(), "the screen is clear again");
        }
    }

    /// The question names what is about to happen and what it is about to happen
    /// to, before it happens rather than after.
    #[test]
    fn the_question_says_what_is_about_to_happen() {
        let (mut acting, _) = choosing('d', a_listing());
        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Accept);

        let said = showing(&acting);

        assert!(said.contains("Stop Full stack?"), "{said}");
        assert!(said.contains("everything, behind the tunnel"), "{said}");
    }

    /// The three actions whose command refuses an empty list are refused where the
    /// stack declares nothing to name, in the words the web surface uses.
    #[test]
    fn an_action_needing_a_form_is_refused_where_the_stack_declares_none() {
        for key in ['s', 't', 'p'] {
            let (acting, _) = choosing(key, nothing_declared());

            let said = showing(&acting);

            assert!(said.contains("needs `forms`"), "{key}: {said}");
        }
    }

    /// Backing out of the list leaves the stack alone and the screen clear.
    #[test]
    fn backing_out_of_the_list_leaves_the_screen_clear() {
        let (mut acting, _) = choosing('p', a_listing());
        assert!(!showing(&acting).is_empty());

        assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);

        assert!(showing(&acting).is_empty());
        assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);
    }

    /// An answer is taken once, by the ask that is outstanding. A second one for the
    /// same question changes nothing, so a reply that arrives twice cannot open a
    /// list over whatever the operator has moved on to.
    #[test]
    fn an_answer_is_taken_once_by_the_ask_that_is_outstanding() {
        let (mut acting, _) = choosing('p', a_listing());
        assert!(showing(&acting).contains("Full stack"));
        acting.pressed(&Press::Abandon);

        acting.told(Ok(Outcome::Forms(a_listing())));

        assert!(showing(&acting).is_empty(), "the ask was already answered");
    }

    /// The keys the screen already answered go on answering, and a key on nothing
    /// is not an action.
    #[test]
    fn the_keys_the_screen_already_answered_still_answer() {
        let mut acting = Acting::opened();

        assert_eq!(acting.pressed(&Press::Typed('r')), Wanted::Gather);
        assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);
        assert_eq!(acting.pressed(&Press::Abandon), Wanted::Leave);
        assert_eq!(acting.pressed(&Press::Typed('z')), Wanted::Nothing);
        assert_eq!(acting.pressed(&Press::Back), Wanted::Nothing);
        assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
    }

    /// The words open on a key, close on any key, and the key that closed them does
    /// nothing else — so putting them away can never start something.
    #[test]
    fn the_words_open_on_a_key_and_the_key_that_closes_them_does_nothing_else() {
        let mut acting = Acting::opened();

        assert_eq!(acting.pressed(&Press::Typed('?')), Wanted::Words);
        assert!(acting.showing_words());

        assert_eq!(acting.pressed(&Press::Typed('d')), Wanted::Nothing);
        assert!(!acting.showing_words());
        assert!(showing(&acting).is_empty(), "no action was begun");
    }

    /// A run that explains nothing has no pane of words to open, and the key that
    /// would have opened it is not an action either.
    #[test]
    fn a_run_that_explains_nothing_opens_no_words() {
        let mut acting = Acting::opened().without_explanations();

        assert_eq!(acting.pressed(&Press::Typed('?')), Wanted::Nothing);
        assert!(!acting.showing_words());
    }

    /// While an action is running, the footer says so and the screen behind it is
    /// left alone — the panels are the report, and covering them would take away
    /// the one thing worth watching.
    #[test]
    fn a_running_action_says_so_on_the_footer_and_covers_nothing() {
        let (mut acting, _) = choosing('t', a_listing());
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Typed('y'));

        assert!(
            showing(&acting).is_empty(),
            "nothing is drawn over the panels"
        );
        let footer = footing(&acting);
        assert!(footer.contains("restart Full stack"), "{footer}");
        assert!(footer.contains("still running"), "{footer}");
    }

    /// Leaving while an action runs is allowed, and the action is still where it
    /// was: what is being waited on is asked for on the way out, because the run
    /// holding the screen is the run carrying the work out. Anything else pressed
    /// leaves it running and says nothing.
    #[test]
    fn leaving_while_an_action_runs_says_what_is_still_being_waited_on() {
        let (mut acting, _) = choosing('t', a_listing());
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Typed('y'));

        assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
        assert!(footing(&acting).contains("still running"));

        assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);
        let said = acting.staying_for().unwrap_or_default();
        assert!(said.contains("restart Full stack"), "{said}");
        assert!(said.contains("leave the stack claimed"), "{said}");
    }

    /// Leaving with nothing running has nothing to wait for, so an operator who
    /// pressed q on an idle screen gets their shell rather than a sentence.
    #[test]
    fn leaving_with_nothing_running_waits_for_nothing() {
        let mut acting = Acting::opened();

        assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);

        assert!(acting.staying_for().is_none());
    }

    /// What an action came to is put on the screen in the words the command line
    /// gives for the same run, and put away by any key.
    #[test]
    fn what_an_action_came_to_is_shown_and_then_put_away() {
        let (mut acting, _) = choosing('t', a_listing());
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Typed('y'));

        acting.came_to(Err(Box::new(a_failure())));

        let said = showing(&acting);
        assert!(said.contains("what it came to"), "{said}");
        assert!(
            said.contains("the container engine could not be reached"),
            "{said}"
        );
        assert!(said.contains("Start the container engine"), "{said}");

        assert_eq!(acting.pressed(&Press::Typed('n')), Wanted::Nothing);
        assert!(showing(&acting).is_empty());
        assert!(footing(&acting).contains("r refresh"));
    }

    /// A stack that will not say what it declares is reported rather than left as a
    /// key that seems to do nothing.
    #[test]
    fn a_stack_that_will_not_say_what_it_declares_says_why() {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed('u'));

        acting.told(Err(Box::new(a_failure())));

        let said = showing(&acting);
        assert!(
            said.contains("the container engine could not be reached"),
            "{said}"
        );
    }

    /// An answer of the wrong shape is said rather than shown as nothing, because a
    /// screen that went quiet reads as an action that never ran.
    #[test]
    fn an_answer_of_the_wrong_shape_is_said_rather_than_swallowed() {
        let mut asked = Acting::opened();
        asked.pressed(&Press::Typed('u'));
        asked.told(Ok(Outcome::Version(a_version())));

        let (mut acted, _) = choosing('t', a_listing());
        acted.pressed(&Press::Accept);
        acted.pressed(&Press::Typed('y'));
        acted.came_to(Ok(Outcome::Version(a_version())));

        let (asked, acted) = (showing(&asked), showing(&acted));
        assert!(asked.contains("something other than"), "{asked}");
        assert!(acted.contains("something other than"), "{acted}");
    }

    /// An answer arriving for a question nobody asked changes nothing, so a stale
    /// reply cannot open a list over whatever the operator has moved on to.
    #[test]
    fn an_answer_nobody_is_waiting_for_changes_nothing() {
        let mut acting = Acting::opened();

        acting.told(Ok(Outcome::Forms(a_listing())));

        assert!(showing(&acting).is_empty());
    }

    /// One keypress as this screen reads it.
    fn read(code: KeyCode, modifiers: KeyModifiers) -> Option<Press> {
        meaning(KeyEvent::new(code, modifiers))
    }

    /// Every key this screen answers arrives as something it can act on, and a key
    /// it has no use for arrives as nothing rather than as a character it never
    /// typed. Ctrl-C is read as backing out: raw mode no longer turns it into a
    /// signal, so an operator who reaches for it is asking to leave.
    #[test]
    fn the_keyboard_reaches_this_screen_as_the_presses_it_answers() {
        assert!(matches!(
            read(KeyCode::Char('q'), KeyModifiers::NONE),
            Some(Press::Typed('q'))
        ));
        assert!(matches!(
            read(KeyCode::Backspace, KeyModifiers::NONE),
            Some(Press::Rubout)
        ));
        assert!(matches!(
            read(KeyCode::Esc, KeyModifiers::NONE),
            Some(Press::Abandon)
        ));
        assert!(matches!(
            read(KeyCode::Enter, KeyModifiers::NONE),
            Some(Press::Accept)
        ));
        assert!(matches!(
            read(KeyCode::Up, KeyModifiers::NONE),
            Some(Press::Back)
        ));
        assert!(matches!(
            read(KeyCode::Down, KeyModifiers::NONE),
            Some(Press::Forward)
        ));
        assert!(matches!(
            read(KeyCode::Char('c'), KeyModifiers::CONTROL),
            Some(Press::Abandon)
        ));
        assert!(read(KeyCode::Home, KeyModifiers::NONE).is_none());
        // A character held with control is not that character: an operator typing
        // ctrl-d on this screen has not asked to stop the stack.
        assert!(read(KeyCode::Char('d'), KeyModifiers::CONTROL).is_none());
    }

    /// Every key an action is on begins that action, and every one of them reaches
    /// the same list of what it can be given — under that action's own name, which
    /// is what proves the answer landed on the offer the key began.
    #[test]
    fn every_key_an_action_is_on_begins_that_action() {
        for (key, named) in [
            ('u', "start"),
            ('d', "stop"),
            ('s', "switch"),
            ('t', "restart"),
            ('p', "fetch"),
        ] {
            let mut acting = Acting::opened();

            let wanted = acting.pressed(&Press::Typed(key));
            acting.told(Ok(Outcome::Forms(a_listing())));

            assert_eq!(wanted, Wanted::Ask(Command::Forms), "{key}");
            let said = showing(&acting);
            assert!(said.contains(named), "{key}: {said}");
        }
    }

    /// The whole flow of a read, which is the claim the other half of this screen
    /// exists to make: one key, a question taken off a list, and a command that is
    /// one of the core's own rather than one assembled here.
    #[test]
    fn a_question_takes_a_key_and_a_choice_before_it_reaches_a_command() {
        let mut acting = Acting::opened();

        assert_eq!(
            acting.pressed(&Press::Typed(question::KEY)),
            Wanted::Nothing
        );
        let said = showing(&acting);
        assert!(said.contains("> versions"), "{said}");
        assert!(said.contains("the container engine"), "{said}");

        assert_eq!(
            acting.pressed(&Press::Accept),
            Wanted::Carry(Command::Version)
        );
        assert!(showing(&acting).contains("waiting for this stack to answer"));
    }

    /// An answer is shown in the words the command line gives for the same request,
    /// rather than in a second account of it — which is the whole reason a question
    /// names a read instead of this screen writing its own report.
    #[test]
    fn an_answer_is_shown_in_the_words_the_command_line_gives() {
        let outcome = Outcome::Version(a_version());
        let printed = crate::render::shaped(&outcome).text();
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(question::KEY));
        acting.pressed(&Press::Accept);

        acting.came_to(Ok(outcome));

        let said = showing(&acting);
        assert!(said.contains("versions"), "the box is named for it: {said}");
        for line in printed.lines().filter(|line| !line.is_empty()) {
            assert!(said.contains(line), "{line:?} is missing from {said}");
        }
    }

    /// A read that could not be carried out says why, in the words the command line
    /// gives for the same failure.
    #[test]
    fn a_question_the_stack_will_not_answer_says_why() {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(question::KEY));
        acting.pressed(&Press::Accept);

        acting.came_to(Err(Box::new(a_failure())));

        let said = showing(&acting);
        assert!(
            said.contains("the container engine could not be reached"),
            "{said}"
        );
    }

    /// A question that has to be given a word gets a line to type it on, takes
    /// characters back one at a time, and only then reaches a command.
    #[test]
    fn a_question_that_takes_a_word_is_typed_before_it_is_asked() {
        let mut acting = asking_where();

        for character in "Expansee".chars() {
            acting.pressed(&Press::Typed(character));
        }
        acting.pressed(&Press::Rubout);
        let said = showing(&acting);
        assert!(said.contains("What to follow"), "{said}");
        assert!(said.contains("> Expanse"), "{said}");

        assert_eq!(
            acting.pressed(&Press::Accept),
            Wanted::Carry(Command::Trace {
                term: "Expanse".to_owned(),
                season: None,
            })
        );
    }

    /// Asking it with nothing typed says what is missing, in the sentence the web
    /// surface gives the same request — the refusal is that surface's, not one this
    /// screen wrote.
    #[test]
    fn a_question_asked_with_no_word_says_what_is_missing() {
        let mut acting = asking_where();

        assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);

        let said = showing(&acting);
        assert!(said.contains(lemonfiber_api::reads::NO_TERM), "{said}");
    }

    /// Moving over the line being typed changes neither it nor the screen, and
    /// backing out of it leaves the screen clear.
    #[test]
    fn the_line_being_typed_ignores_a_move_and_is_left_on_a_way_out() {
        let mut acting = asking_where();
        acting.pressed(&Press::Typed('x'));

        assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
        assert!(showing(&acting).contains("> x"));

        assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
        assert!(showing(&acting).is_empty());
    }

    /// Moving over the questions and typing at them take nothing, the way the list
    /// of what an action can be given does.
    #[test]
    fn moving_over_the_questions_and_typing_at_them_take_nothing() {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(question::KEY));

        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Back);
        assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);
        assert_eq!(acting.pressed(&Press::Rubout), Wanted::Nothing);
        assert!(showing(&acting).contains("> versions"));

        assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
        assert!(showing(&acting).is_empty());
    }

    /// A question with the core waits for it, and can be left the way the listing
    /// of an action's subjects can — after which its answer changes nothing, so a
    /// reply nobody is waiting for cannot open a box over what came next.
    #[test]
    fn a_question_left_before_it_lands_takes_the_answer_nowhere() {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(question::KEY));
        acting.pressed(&Press::Accept);

        assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
        assert!(showing(&acting).contains("waiting for this stack to answer"));

        assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
        acting.came_to(Ok(Outcome::Version(a_version())));

        assert!(showing(&acting).is_empty());
    }

    /// An answer longer than the box moves through it, and any key that is not a
    /// move puts it away — the same dismissal the pane of words has.
    #[test]
    fn an_answer_moves_under_the_arrows_and_closes_under_anything_else() {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(question::KEY));
        acting.pressed(&Press::Accept);
        acting.came_to(Ok(Outcome::Version(a_version())));
        let opened = showing(&acting);

        assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
        let moved = showing(&acting);
        assert!(moved.contains("1 more line above"), "{moved}");
        assert_ne!(moved, opened);

        acting.pressed(&Press::Back);
        assert_eq!(showing(&acting), opened);

        assert_eq!(acting.pressed(&Press::Typed('n')), Wanted::Nothing);
        assert!(showing(&acting).is_empty());
    }

    /// What an action came to moves the same way, so two boxes an operator meets a
    /// keypress apart do not answer the arrows differently.
    #[test]
    fn what_an_action_came_to_moves_the_same_way() {
        let (mut acting, _) = choosing('t', a_listing());
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Typed('y'));
        acting.came_to(Err(Box::new(a_failure())));
        let opened = showing(&acting);

        acting.pressed(&Press::Forward);

        assert_ne!(showing(&acting), opened);
        assert!(showing(&acting).contains("1 more line above"));
    }

    /// The screen, having taken one errand off the list the `more` key opens.
    fn sending(action: &str) -> (Acting, Wanted) {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(errand::KEY));
        let named = errand::tests::listed(action);
        while !showing(&acting).contains(&format!("> {named}")) {
            acting.pressed(&Press::Forward);
        }
        let wanted = acting.pressed(&Press::Accept);
        (acting, wanted)
    }

    /// What a reset would revert, or did.
    fn a_reset(confirmed: bool) -> ResetReport {
        ResetReport {
            reverted: vec![StackEdit {
                path: "compose.yaml".to_owned(),
                diff: "-yours\n+ours".to_owned(),
            }],
            reverted_connections: vec!["sonarr → sabnzbd".to_owned()],
            confirmed,
        }
    }

    /// What a capture produced.
    fn a_capture() -> backup::Report {
        backup::Report {
            path: PathBuf::from("/data/lemonfiber/backups/lemonfiber-full-1.tar.gz"),
            scope: Scope::WholeStack,
            sensitive: false,
            pruned: Vec::new(),
        }
    }

    /// The other half of what this screen exists to prove: an errand takes the key
    /// that opens the rest of them, a choice off that list, and an explicit yes
    /// before anything reaches a command — and the command is one of the core's own.
    #[test]
    fn an_errand_takes_a_key_a_choice_and_an_answer_before_it_reaches_a_command() {
        let (mut acting, opened) = sending("seed");
        assert_eq!(opened, Wanted::Nothing);

        let said = showing(&acting);
        assert!(said.contains("Wire the services to each other?"), "{said}");

        assert_eq!(
            acting.pressed(&Press::Typed('y')),
            Wanted::Carry(Command::Seed)
        );
    }

    /// The claim the destructive errands are built around: what would be lost is on
    /// the screen before the question is put, not after the answer is given.
    #[test]
    fn a_reset_says_what_it_would_throw_away_before_it_asks() {
        let (mut acting, weighing) = sending("reset");
        assert_eq!(weighing, Wanted::Carry(Command::Reset { confirm: false }));

        acting.came_to(Ok(Outcome::Reset(a_reset(false))));

        let said = showing(&acting);
        let before = said
            .split("Throw away every edit above?")
            .next()
            .unwrap_or_default();
        assert!(before.contains("sonarr → sabnzbd"), "{said}");
        assert!(before.contains("compose.yaml"), "{said}");
        assert_eq!(
            acting.pressed(&Press::Typed('y')),
            Wanted::Carry(Command::Reset { confirm: true })
        );
    }

    /// The same reading for a restore, which overwrites a configuration rather than
    /// discarding edits: the archive is named, what it holds is read, and only then
    /// is the overwrite agreed to.
    #[test]
    fn a_restore_is_named_read_and_only_then_agreed_to() {
        let (mut acting, opened) = sending("restore");
        assert_eq!(opened, Wanted::Nothing);
        assert!(showing(&acting).contains("Which backup"));

        for character in "lemonfiber-full-1.tar.gz".chars() {
            acting.pressed(&Press::Typed(character));
        }
        let listing = acting.pressed(&Press::Accept);

        assert_eq!(
            listing,
            Wanted::Carry(Command::Restore {
                archive: Kept::Named("lemonfiber-full-1.tar.gz".to_owned()),
                repoint: false,
                confirm: false,
            })
        );
    }

    /// A restore asked for with nothing typed is refused in the words the web
    /// surface gives for the same request, rather than in a sentence written here.
    #[test]
    fn a_restore_with_no_name_says_what_is_missing() {
        let (mut acting, _) = sending("restore");

        assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);

        let said = showing(&acting);
        assert!(said.contains("restore"), "{said}");
        assert!(said.contains("archive"), "{said}");
    }

    /// What an errand came to is shown in the words the command line gives for the
    /// same run, and the screen behind it goes on being the report while it runs.
    #[test]
    fn an_errand_under_way_covers_nothing_and_says_what_it_came_to() {
        let (mut acting, _) = sending("backup");
        acting.pressed(&Press::Typed('y'));

        assert!(acting.pane(20, 100).is_none());
        let footing = footing(&acting);
        assert!(footing.contains("a backup"), "{footing}");
        assert!(
            acting
                .staying_for()
                .is_some_and(|said| said.contains("a backup")),
            "an errand with the core is waited for"
        );

        acting.came_to(Ok(Outcome::Backup(a_capture())));

        let said = showing(&acting);
        assert!(said.contains("Backed up"), "{said}");
    }

    /// An errand that failed says so in the command line's own words, the way an
    /// action that failed does — a screen that went quiet would read as an errand
    /// that never ran.
    #[test]
    fn an_errand_that_failed_says_why_in_the_words_the_command_line_gives() {
        let (mut acting, _) = sending("seed");
        acting.pressed(&Press::Typed('y'));

        acting.came_to(Err(Box::new(a_failure())));

        let said = showing(&acting);
        assert!(said.contains("could not be reached"), "{said}");
    }

    /// An errand with the core can be left, and leaving does not stop it — the same
    /// reading a running action gets, because the same process holds the claim.
    #[test]
    fn leaving_while_an_errand_runs_leaves_the_screen_and_not_the_run() {
        let (mut acting, _) = sending("seed");
        acting.pressed(&Press::Typed('y'));

        assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
        assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);
    }

    /// Anything that is not an explicit yes leaves the stack as it is, and the box
    /// closes rather than staying open over a decision already taken.
    #[test]
    fn an_errand_answered_with_anything_else_changes_nothing() {
        let (mut acting, _) = sending("seed");

        assert_eq!(acting.pressed(&Press::Typed('n')), Wanted::Nothing);

        assert!(showing(&acting).is_empty());
    }

    /// What a reset would do moves under the arrows, and moving is not agreeing:
    /// the question is still there afterwards and nothing has been sent.
    #[test]
    fn what_an_errand_would_do_moves_without_agreeing_to_it() {
        let (mut acting, _) = sending("reset");
        acting.came_to(Ok(Outcome::Reset(a_reset(false))));
        let opened = showing(&acting);

        assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);

        let moved = showing(&acting);
        assert_ne!(moved, opened);
        assert!(moved.contains("Throw away every edit above?"), "{moved}");
        assert!(moved.contains("1 more line above"), "{moved}");
    }

    /// Backing out of the list, the line and the wait each leave the screen clear,
    /// so no half-answered errand is left open behind whatever came next.
    #[test]
    fn backing_out_of_an_errand_leaves_the_screen_clear() {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(errand::KEY));
        assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
        assert!(showing(&acting).is_empty());

        let (mut acting, _) = sending("restore");
        assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
        assert!(showing(&acting).is_empty());

        let (mut acting, _) = sending("reset");
        assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
        assert!(showing(&acting).contains("working out what this would do"));
        assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
        assert!(showing(&acting).is_empty());
    }

    /// Moving over the errands and typing at them take nothing, the way moving over
    /// an action's subjects does: a stray key over a list is not an answer to it.
    #[test]
    fn moving_over_the_errands_and_typing_at_them_take_nothing() {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(errand::KEY));

        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Back);
        assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);

        let said = showing(&acting);
        assert!(said.contains("> wiring"), "{said}");
    }

    /// The line an errand is named on ignores a move and takes back what was typed,
    /// the way the line a question is typed on does.
    #[test]
    fn the_line_an_errand_is_named_on_takes_back_what_was_typed() {
        let (mut acting, _) = sending("restore");

        acting.pressed(&Press::Typed('a'));
        acting.pressed(&Press::Typed('b'));
        acting.pressed(&Press::Rubout);
        acting.pressed(&Press::Forward);

        let said = showing(&acting);
        assert!(said.contains("> a"), "{said}");
        assert!(!said.contains("> ab"), "{said}");
    }

    /// A stack that will not say what an errand would do ends the errand there:
    /// there is nothing to agree to, and the failure is said in the command line's
    /// own words.
    #[test]
    fn an_errand_the_stack_will_not_weigh_says_why_and_asks_nothing() {
        let (mut acting, _) = sending("reset");

        acting.came_to(Err(Box::new(a_failure())));

        let said = showing(&acting);
        assert!(said.contains("could not be reached"), "{said}");
        assert!(!said.contains("Throw away"), "{said}");
    }

    /// An errand naming an action no surface offers reaches no command and says so.
    /// Nothing on the list is one, and that is what the guard beside the list holds;
    /// this is the arm that would carry a name that stopped being offered.
    #[test]
    fn an_errand_naming_an_action_nothing_offers_says_so() {
        let mut acting = Acting::opened();

        let wanted = errand::agreeing(
            &mut acting.stage,
            &errand::tests::UNTRANSLATABLE,
            String::new(),
            None,
            &Press::Typed('y'),
        );

        assert_eq!(wanted, Wanted::Nothing);
        let said = showing(&acting);
        assert!(said.contains("There is no action named"), "{said}");
    }

    /// The screen, having taken one quality change off the list its key opens.
    fn changing(action: &str) -> (Acting, Wanted) {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(quality::KEY));
        let named = quality::tests::listed(action);
        while !showing(&acting).contains(&format!("> {named}")) {
            acting.pressed(&Press::Forward);
        }
        let wanted = acting.pressed(&Press::Accept);
        (acting, wanted)
    }

    /// The quality in force as the read reports it, and what became of a choice.
    ///
    /// The preset's own words rather than a paraphrase, so what is asserted about
    /// the screen is what the core would really have put on it.
    fn a_quality(disposition: Disposition) -> QualityReport {
        let cost = Preset::Maximum.consequence();
        QualityReport {
            choices: vec![PresetChoice {
                scope: "everything".to_owned(),
                preset: Preset::Maximum.label().to_owned(),
                means: Preset::Maximum.means().to_owned(),
                resolution: cost.resolution.to_owned(),
                size_per_hour: cost.size_per_hour.to_owned(),
                transcoding: cost.transcoding.to_owned(),
                needs_transcoding_here: true,
            }],
            music: None,
            customised: false,
            disposition,
        }
    }

    /// What an upgrade would cost, having triggered nothing.
    fn an_upgrade() -> UpgradeReport {
        UpgradeReport {
            confirmed: false,
            media: vec![UpgradeMedia {
                media_type: "television".to_owned(),
                preset: Preset::Balanced.label().to_owned(),
                size_per_hour: Preset::Balanced.consequence().size_per_hour.to_owned(),
                outcome: None,
            }],
        }
    }

    /// The same claim as the actions and the errands, on the third list: a key, a
    /// choice, a question and an explicit yes before anything reaches a command —
    /// and the command is the one every other surface produces for that request.
    #[test]
    fn a_quality_change_takes_a_key_a_choice_and_an_answer_before_it_reaches_a_command() {
        let (mut acting, opened) = changing("quality-set");
        assert_eq!(opened, Wanted::Nothing);

        let listed = showing(&acting);
        assert!(listed.contains("> space-saving"), "{listed}");
        assert!(listed.contains("per hour"), "{listed}");

        acting.pressed(&Press::Forward);
        assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
        let asked = showing(&acting);
        assert!(asked.contains("Aim for balanced?"), "{asked}");

        assert_eq!(
            acting.pressed(&Press::Typed('y')),
            Wanted::Carry(Command::Quality(QualityAction::Set {
                preset: Preset::Balanced,
                media_type: None,
                confirm: false,
            }))
        );
    }

    /// What an upgrade would cost is on the screen before the question is put, not
    /// after the answer is given — the reading a reset is already offered under,
    /// arriving on the one of these three whose unconfirmed run really does report
    /// and change nothing.
    #[test]
    fn what_an_upgrade_would_cost_is_read_before_it_is_agreed_to() {
        let (mut acting, costing) = changing("quality-upgrade");
        assert_eq!(
            costing,
            Wanted::Carry(Command::QualityUpgrade { confirm: false })
        );

        acting.came_to(Ok(Outcome::Upgrade(an_upgrade())));

        let said = showing(&acting);
        let before = said
            .split("Fetch everything again at the quality chosen?")
            .next()
            .unwrap_or_default();
        assert!(before.contains("television"), "{said}");
        assert!(
            before.contains(Preset::Balanced.consequence().size_per_hour),
            "{said}"
        );
        assert!(before.contains("Nothing has been changed"), "{said}");
        assert_eq!(
            acting.pressed(&Press::Typed('y')),
            Wanted::Carry(Command::QualityUpgrade { confirm: true })
        );

        acting.came_to(Ok(Outcome::Upgrade(UpgradeReport {
            confirmed: true,
            media: an_upgrade().media,
        })));

        let came = showing(&acting);
        assert!(came.contains("Upgrading existing content"), "{came}");
        assert!(!came.contains("Fetch everything again"), "{came}");
    }

    /// A choice this host could only transcode in software comes back held rather
    /// than recorded, and the caution it comes back with is what the second question
    /// sits under. The agreement goes on that second send and never on the first.
    #[test]
    fn a_choice_this_host_would_transcode_is_held_and_the_caution_is_read_first() {
        let (mut acting, _) = changing("quality-set");
        while !showing(&acting).contains("> maximum") {
            acting.pressed(&Press::Forward);
        }
        acting.pressed(&Press::Accept);
        let first = acting.pressed(&Press::Typed('y'));
        assert_eq!(
            first,
            Wanted::Carry(Command::Quality(QualityAction::Set {
                preset: Preset::Maximum,
                media_type: None,
                confirm: false,
            }))
        );

        acting.came_to(Ok(Outcome::Quality(a_quality(Disposition::Held))));

        let said = showing(&acting);
        let before = said.split("Aim for maximum?").next().unwrap_or_default();
        assert!(before.contains("transcode this in software"), "{said}");
        assert_eq!(
            acting.pressed(&Press::Typed('y')),
            Wanted::Carry(Command::Quality(QualityAction::Set {
                preset: Preset::Maximum,
                media_type: None,
                confirm: true,
            }))
        );
    }

    /// A choice that was recorded is read and put away, not asked about again — the
    /// second question exists for the one cost the agreement is for and for nothing
    /// else.
    #[test]
    fn a_choice_that_was_recorded_is_read_rather_than_asked_about_again() {
        let (mut acting, _) = changing("quality-set");
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Typed('y'));

        acting.came_to(Ok(Outcome::Quality(a_quality(Disposition::Recorded))));

        let said = showing(&acting);
        assert!(said.contains("Saved."), "{said}");
        assert!(!said.contains("Aim for"), "{said}");
        assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);
    }

    /// Re-asserting carries no agreement and has no half that only reports, so the
    /// question is put with nothing above it rather than with a preamble invented
    /// for symmetry — and what it would overwrite is the line under the question.
    #[test]
    fn re_asserting_the_choice_is_asked_about_with_nothing_run_first() {
        let (mut acting, opened) = changing("quality-reapply");
        assert_eq!(opened, Wanted::Nothing);

        let said = showing(&acting);
        assert!(
            said.contains("Overwrite your own edits with the chosen preset?"),
            "{said}"
        );
        assert!(said.contains("edited by hand"), "{said}");
        assert!(!said.contains("more line"), "{said}");

        assert_eq!(
            acting.pressed(&Press::Typed('y')),
            Wanted::Carry(Command::Quality(QualityAction::Reapply))
        );
    }

    /// Moving over either quality list lands where it started, and a stray character
    /// pressed over one is not an answer to it.
    #[test]
    fn moving_over_the_quality_lists_and_typing_at_them_take_nothing() {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(quality::KEY));

        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Back);
        acting.pressed(&Press::Rubout);
        assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);
        assert!(showing(&acting).contains("> how good it should be"));

        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Back);
        acting.pressed(&Press::Rubout);
        acting.pressed(&Press::Typed('z'));
        assert!(showing(&acting).contains("> space-saving"));

        acting.pressed(&Press::Abandon);
        assert_eq!(showing(&acting), "");
    }

    /// Backing out of the list of changes leaves nothing open, and a question left
    /// alone leaves the quality as it was.
    #[test]
    fn backing_out_of_a_quality_change_leaves_it_as_it_was() {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(quality::KEY));
        acting.pressed(&Press::Abandon);
        assert_eq!(showing(&acting), "");

        let (mut acting, _) = changing("quality-reapply");
        assert_eq!(acting.pressed(&Press::Typed('n')), Wanted::Nothing);
        assert_eq!(showing(&acting), "");
    }

    /// The account in front of the question moves, because a cost stated per media
    /// type is longer than the box over a dashboard and one whose end nobody can
    /// reach is one nobody has read.
    #[test]
    fn the_account_in_front_of_the_question_moves() {
        let (mut acting, _) = changing("quality-upgrade");
        acting.came_to(Ok(Outcome::Upgrade(an_upgrade())));
        let opened = showing(&acting);

        acting.pressed(&Press::Forward);

        assert_ne!(showing(&acting), opened);
        assert!(showing(&acting).contains("1 more line above"));
        acting.pressed(&Press::Back);
        assert_eq!(showing(&acting), opened);
    }

    /// While a cost is with the core the screen says so, and backing out then leaves
    /// nothing open — the answer that arrives afterwards has nobody waiting for it.
    #[test]
    fn a_cost_being_worked_out_is_said_and_can_be_backed_out_of() {
        let (mut acting, _) = changing("quality-upgrade");
        let said = showing(&acting);
        assert!(said.contains("working out what this would cost"), "{said}");

        assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);
        assert_eq!(showing(&acting), said);

        acting.pressed(&Press::Abandon);
        assert_eq!(showing(&acting), "");
        acting.came_to(Ok(Outcome::Upgrade(an_upgrade())));
        assert_eq!(showing(&acting), "");
    }

    /// A change that is running draws no box, says what it is on the footer, and
    /// tells a run leaving now what it would be waiting for.
    #[test]
    fn a_running_quality_change_is_on_the_footer_and_leaving_says_what_it_waits_for() {
        let (mut acting, _) = changing("quality-reapply");
        acting.pressed(&Press::Typed('y'));

        assert_eq!(showing(&acting), "");
        let footing = footing(&acting);
        assert!(
            footing.contains("the choice put back over your edits"),
            "{footing}"
        );
        assert!(footing.contains("still running"), "{footing}");
        let leaving = acting.staying_for().unwrap_or_default();
        assert!(leaving.contains("leave the stack claimed"), "{leaving}");

        assert_eq!(acting.pressed(&Press::Typed('z')), Wanted::Nothing);
        assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);
    }

    /// A failure on either half is reported in the words the command line gives for
    /// the same one, and nothing is left to agree to.
    #[test]
    fn a_failure_while_a_quality_change_is_with_the_core_is_reported() {
        let (mut acting, _) = changing("quality-upgrade");
        acting.came_to(Err(Box::new(a_failure())));
        let costing = showing(&acting);
        assert!(costing.contains("could not be reached"), "{costing}");
        assert!(!costing.contains("Fetch everything again"), "{costing}");

        let (mut acting, _) = changing("quality-reapply");
        acting.pressed(&Press::Typed('y'));
        acting.came_to(Err(Box::new(a_failure())));
        let applying = showing(&acting);
        assert!(applying.contains("could not be reached"), "{applying}");
    }

    /// A change naming an action no surface offers reaches no command and says so,
    /// on each of the three paths that ask the translation for one. Nothing on the
    /// list is one, and the guard beside the list is what holds that; these are the
    /// arms that would carry a name which stopped being offered.
    #[test]
    fn a_quality_change_naming_an_action_nothing_offers_says_so() {
        for change in [&quality::tests::UNCHOOSABLE, &quality::tests::UNCOSTABLE] {
            let mut acting = Acting::opened();

            let wanted = quality::deciding(
                &mut acting.stage,
                Chooser::over(change, Vec::new()),
                &Press::Accept,
            );

            assert_eq!(wanted, Wanted::Nothing);
            let said = showing(&acting);
            assert!(said.contains("There is no action named"), "{said}");
        }

        let mut acting = Acting::opened();
        let wanted = quality::settling(
            &mut acting.stage,
            &quality::tests::UNCHOOSABLE,
            None,
            None,
            &Press::Typed('y'),
        );

        assert_eq!(wanted, Wanted::Nothing);
        assert!(showing(&acting).contains("There is no action named"));
    }

    /// Every request this screen reaches is one of the lists it is built from, and
    /// every entry on those lists is a request it reaches. What the parity table's
    /// terminal column is held to is this list, so a screen that grew an offer
    /// nothing published would leave that column quietly short.
    #[test]
    fn what_this_screen_reaches_is_what_it_publishes() {
        let published = lemonfiber::reaching::reached();

        for request in [
            "up",
            "seed",
            "reset",
            "restore",
            "version",
            "trace",
            "walkthrough",
            "watch",
            "ui",
        ] {
            assert!(published.contains(&request), "{request} is not published");
        }
    }

    /// The screen, having taken one of the two that keep going off its own list.
    fn starting(action: &str) -> Acting {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(lasting::KEY));
        let named = lasting::tests::listed(action);
        while !showing(&acting).contains(&format!("> {named}")) {
            acting.pressed(&Press::Forward);
        }
        acting.pressed(&Press::Accept);
        acting
    }

    /// A walk takes the key, a line to type on and an explicit yes before it reaches
    /// a command — and the command is one of the core's own, carrying what was typed.
    #[test]
    fn a_walk_takes_a_key_a_word_and_an_answer_before_it_reaches_a_command() {
        let mut acting = starting("walkthrough");
        assert!(showing(&acting).contains("What to look for"));

        for character in "Sintel".chars() {
            acting.pressed(&Press::Typed(character));
        }
        assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
        let asking = showing(&acting);
        assert!(asking.contains("Walk through Sintel?"), "{asking}");

        assert_eq!(
            acting.pressed(&Press::Typed('y')),
            Wanted::Carry(Command::Walkthrough {
                item: Some("Sintel".to_owned())
            })
        );
    }

    /// Naming nothing is a request rather than a half-finished one, so it is put as
    /// one — and what it reaches is the walk that suggests something.
    #[test]
    fn a_walk_asked_for_nothing_is_asked_about_as_its_own_request() {
        let mut acting = starting("walkthrough");

        acting.pressed(&Press::Accept);
        let asking = showing(&acting);

        assert!(asking.contains("Find something"), "{asking}");
        assert!(!asking.contains("Walk through?"), "{asking}");
        assert_eq!(
            acting.pressed(&Press::Typed('y')),
            Wanted::Carry(Command::Walkthrough { item: None })
        );
    }

    /// A walk's steps are put on the screen as they arrive, in the words a shell is
    /// given for the same step — and the one that is kept for the report is not said
    /// aloud here either.
    #[test]
    fn a_walk_says_each_step_in_the_words_a_shell_is_given_for_it() {
        let mut acting = starting("walkthrough");
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Typed('y'));

        acting.stepped(&Step::at(WalkStep::Choosing));
        assert!(showing(&acting).contains("asking the services"));

        acting.stepped(&Step::searched(3, 47));
        let said = showing(&acting);

        assert!(said.contains("Searching indexers"), "{said}");
        assert!(said.contains("47 releases"), "{said}");
        assert!(!said.contains("asking the services"), "{said}");
    }

    /// And what it came to goes under them rather than over them, which is the order
    /// a shell shows them in.
    #[test]
    fn what_a_walk_came_to_goes_under_the_steps_that_were_watched() {
        let mut acting = starting("walkthrough");
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Typed('y'));
        acting.stepped(&Step::searched(3, 47));

        acting.came_to(Err(Box::new(a_failure())));
        let said = showing(&acting);

        assert!(said.contains("Searching indexers"), "{said}");
        assert!(said.contains("could not be reached"), "{said}");
    }

    /// A walk naming an action no surface offers reaches no command and says so.
    /// Nothing on the list is one, and that is what the guard beside the list holds;
    /// this is the arm that would carry a name that stopped being offered.
    #[test]
    fn a_walk_naming_an_action_nothing_offers_says_so() {
        let mut acting = Acting::opened();

        let wanted = lasting::beginning(
            &mut acting.stage,
            &lasting::tests::NOTHING_ANSWERS,
            lasting::Begun::Looked("Sintel".to_owned()),
            &Press::Typed('y'),
        );

        assert_eq!(wanted, Wanted::Nothing);
        let said = showing(&acting);
        assert!(said.contains("There is no action named"), "{said}");
    }

    /// A guard is chosen one of the stack's own forms, and the whole stack is not
    /// among them: the translation refuses a guard with nothing to stop, so that
    /// choice never reaches the operator. Nothing here decides that a second time.
    #[test]
    fn a_guard_is_offered_the_forms_and_never_the_whole_stack() {
        let mut acting = starting("watch");
        acting.told(Ok(Outcome::Forms(a_listing())));
        let offered = showing(&acting);

        assert!(offered.contains("Full stack"), "{offered}");
        assert!(offered.contains("Lean stack"), "{offered}");
        assert!(
            !offered.contains("the whole stack, rather than one form"),
            "{offered}"
        );
    }

    /// A stack declaring no forms leaves a guard nothing to be given, and the words
    /// the operator is refused in are the web surface's own.
    #[test]
    fn a_guard_over_a_stack_that_declares_no_forms_is_refused_in_the_webs_words() {
        let mut acting = starting("watch");

        acting.told(Ok(Outcome::Forms(nothing_declared())));
        let said = showing(&acting);

        assert!(said.contains("forms"), "{said}");
    }

    /// The screen, with a guard running over one form.
    fn guarding() -> Acting {
        let mut acting = starting("watch");
        acting.told(Ok(Outcome::Forms(a_listing())));
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Typed('y'));
        acting
    }

    /// A guard takes the same three steps and reaches the core's own command.
    #[test]
    fn a_guard_takes_a_key_a_form_and_an_answer_before_it_reaches_a_command() {
        let mut acting = starting("watch");
        acting.told(Ok(Outcome::Forms(a_listing())));
        assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
        let asking = showing(&acting);
        assert!(asking.contains("Guard the data location"), "{asking}");

        assert_eq!(
            acting.pressed(&Press::Typed('y')),
            Wanted::Carry(Command::Watch {
                forms: vec!["full".to_owned()]
            })
        );
    }

    /// The one thing on this screen with no ending of its own is the one the screen
    /// offers to end, and an interruption is what ends it — which on a terminal in
    /// raw mode is a keypress rather than a signal.
    #[test]
    fn an_interruption_lets_a_guard_go_and_says_what_it_left_alone() {
        let mut acting = guarding();

        assert_eq!(acting.pressed(&Press::Abandon), Wanted::Stop);

        let said = showing(&acting);
        assert!(said.contains("let go"), "{said}");
        assert!(said.contains("as they were"), "{said}");
        assert_eq!(
            acting.staying_for(),
            None,
            "a guard that has been let go is not something to wait for"
        );
    }

    /// And leaving does not end it. The stage is still the guard, so the run is
    /// still holding the task — and what is said on the way out says it will not end
    /// by itself and what does end it.
    #[test]
    fn leaving_a_guard_leaves_it_guarding_and_says_so() {
        let mut acting = guarding();

        assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);

        let leaving = acting.staying_for().unwrap_or_default();
        assert!(leaving.contains("still running"), "{leaving}");
        assert!(leaving.contains("data location is lost"), "{leaving}");
        assert!(leaving.contains("Ctrl-C ends it"), "{leaving}");
    }

    /// A walk ends by itself, so the screen offers no end for it: an interruption
    /// leaves the screen the way it does for every other running thing here, and the
    /// run waits for the walk on the ordinary terminal.
    #[test]
    fn a_walk_is_left_running_rather_than_offered_an_end() {
        let mut acting = starting("walkthrough");
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Typed('y'));

        assert_eq!(acting.pressed(&Press::Abandon), Wanted::Leave);

        let leaving = acting.staying_for().unwrap_or_default();
        assert!(leaving.contains("waiting for a walk through"), "{leaving}");
        assert!(!leaving.contains("stack claimed"), "{leaving}");
    }

    /// What a guard is doing is said on the footer rather than drawn over the panels
    /// it is guarding, and a walk's steps are drawn because nothing behind them is
    /// showing them.
    #[test]
    fn a_guard_leaves_the_panels_showing_and_a_walk_does_not() {
        let guard = guarding();
        assert_eq!(guard.pane(20, 100).map(|pane| pane.title), None);
        let footing = footing(&guard);
        assert!(footing.contains("esc lets it go"), "{footing}");
        assert!(footing.contains("leaves it guarding"), "{footing}");

        let mut walk = starting("walkthrough");
        walk.pressed(&Press::Accept);
        walk.pressed(&Press::Typed('y'));
        assert!(walk.pane(20, 100).is_some());
    }

    /// A walk that has said more than the box holds is moved through rather than
    /// cut, the way every other answer on this screen is.
    #[test]
    fn a_walk_that_outgrows_its_box_is_moved_through() {
        let mut acting = starting("walkthrough");
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Typed('y'));
        for _ in 0..12 {
            acting.stepped(&Step::searched(3, 47));
        }

        assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);

        assert!(showing(&acting).contains("1 more line above"));
    }

    /// A step said while nothing is walking changes nothing, which is what a walk
    /// left running and then let go of would say into.
    #[test]
    fn a_step_said_with_no_walk_open_changes_nothing() {
        let mut acting = Acting::opened();

        acting.stepped(&Step::searched(3, 47));

        assert_eq!(showing(&acting), String::new());
    }

    /// Each of the three lists is opened by its own key, and backing out of any of
    /// them leaves the screen as it was.
    #[test]
    fn backing_out_of_the_list_that_keeps_going_leaves_the_screen_alone() {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(lasting::KEY));
        assert!(showing(&acting).contains("keeps going"));
        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Back);
        assert!(showing(&acting).contains("> a walk through"));

        assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);

        assert_eq!(showing(&acting), String::new());
    }

    /// And so does backing out of what either of them is given before the question.
    #[test]
    fn backing_out_of_what_one_is_given_leaves_the_screen_alone() {
        let mut walking = starting("walkthrough");
        assert_eq!(walking.pressed(&Press::Abandon), Wanted::Nothing);
        assert_eq!(showing(&walking), String::new());

        let mut guarding = starting("watch");
        guarding.told(Ok(Outcome::Forms(a_listing())));
        assert_eq!(guarding.pressed(&Press::Abandon), Wanted::Nothing);
        assert_eq!(showing(&guarding), String::new());
    }

    /// A key that means nothing where it was pressed changes nothing there — on the
    /// list, at the question, and while something is running. A screen that acted on
    /// a stray letter would be a screen nobody could lean on.
    #[test]
    fn a_key_that_means_nothing_where_it_was_pressed_changes_nothing() {
        let mut listing = Acting::opened();
        listing.pressed(&Press::Typed(lasting::KEY));
        assert_eq!(listing.pressed(&Press::Typed('z')), Wanted::Nothing);
        assert!(showing(&listing).contains("keeps going"));

        let mut asking = starting("walkthrough");
        asking.pressed(&Press::Accept);
        assert_eq!(asking.pressed(&Press::Typed('n')), Wanted::Nothing);
        assert_eq!(showing(&asking), String::new());

        let mut running = guarding();
        assert_eq!(running.pressed(&Press::Typed('z')), Wanted::Nothing);
        let footing = footing(&running);
        assert!(footing.contains("esc lets it go"), "{footing}");
    }

    /// What a walk is told to look for is typed, taken back and moved over the way
    /// every other line on this screen is.
    #[test]
    fn the_line_a_walk_is_typed_on_takes_back_what_was_typed() {
        let mut acting = starting("walkthrough");

        for character in "Sintelx".chars() {
            acting.pressed(&Press::Typed(character));
        }
        acting.pressed(&Press::Rubout);
        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Back);
        acting.pressed(&Press::Accept);

        assert!(showing(&acting).contains("Walk through Sintel?"));
    }

    /// And the forms a guard could be given are moved over the way every other list
    /// on this screen is.
    #[test]
    fn the_forms_a_guard_could_be_given_are_moved_over_like_any_list() {
        let mut acting = starting("watch");
        acting.told(Ok(Outcome::Forms(a_listing())));

        acting.pressed(&Press::Forward);
        acting.pressed(&Press::Typed('z'));
        acting.pressed(&Press::Back);
        acting.pressed(&Press::Accept);

        assert!(showing(&acting).contains("Guard the data location while running Full stack?"));
    }

    /// What a guard came to is the whole of its box, because it had said nothing
    /// until then — and it is rendered by the renderer the command line reaches for
    /// the same answer, whether it ended or failed.
    #[test]
    fn what_a_guard_came_to_is_the_whole_of_its_box() {
        let mut failed = guarding();
        failed.came_to(Err(Box::new(a_failure())));
        let said = showing(&failed);
        assert!(said.contains("could not be reached"), "{said}");
        assert!(said.contains("what it came to"), "{said}");

        let mut ended = guarding();
        ended.came_to(Ok(Outcome::Watch(SupervisionReport {
            forms: vec!["full".to_owned()],
            reason: "the data location is no longer present".to_owned(),
            stopped: true,
        })));
        let ending = showing(&ended);
        assert!(ending.contains("no longer present"), "{ending}");
    }

    /// The question before the terminal is handed over names what it costs, since
    /// what it costs is the screen somebody was reading.
    #[test]
    fn the_key_that_hands_the_terminal_over_says_so_before_it_does() {
        let mut acting = Acting::opened();

        acting.pressed(&Press::Typed(surface::KEY));

        let said = showing(&acting);
        assert!(said.contains("web interface"), "{said}");
        assert!(said.contains("Close this screen"), "{said}");
        assert!(said.contains("any other key changes nothing"), "{said}");
    }

    /// The footer names every key the screen answers, including the two this slice
    /// added, since a key nobody is told about is a key nobody presses.
    #[test]
    fn the_footer_names_the_keys_that_keep_going_and_the_one_that_hands_over() {
        let footing = footing(&Acting::opened());

        assert!(footing.contains(&format!("{} keeps going", lasting::KEY)));
        assert!(footing.contains(&format!("{} web", surface::KEY)));
    }
}
