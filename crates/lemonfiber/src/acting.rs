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
//! **A long action reports through the screen it interrupted.** The web answers one
//! with a job's name because a request cannot be held open for minutes; a terminal
//! has no such indirection and needs none, because the dashboard is already the
//! report. The panels go on gathering every second while the work runs, so a
//! restart shows as the services going down and coming back, in the panel that
//! lists them. Nothing is drawn over that — only the footer says what is running.
//!
//! **Leaving closes the screen, not the run.** A closed browser tab leaves a server
//! carrying the job on; a closed dashboard has no server to leave it to. The process
//! drawing this screen is the one that claimed the stack and issued the command, and
//! it is the only one that can give the stack back — so leaving gives the screen back
//! at once and the run stays until the action it started has finished. Saying
//! otherwise would leave an operator to find out from the next command, refused in
//! the name of a process that no longer exists.

mod chooser;
mod offer;
mod question;
mod reading;
mod words;

use lemonfiber_core::app::{Command, Outcome};
use lemonfiber_core::error::Problem;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Line;

use chooser::Chooser;
use offer::{Choice, Offer};
use question::Question;
use reading::Reading;

/// The key that opens the list of what this stack can be asked.
///
/// Re-exported for the screen these boxes are drawn over, whose own tests press it
/// rather than writing the letter down a second time.
#[cfg(test)]
pub(crate) use question::KEY as ASK;
pub(crate) use words::Pane;

/// What is said where an answer is not the shape the question had.
const NOT_THE_ANSWER: &str = "This stack answered something other than what was asked of it.";

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
    /// Leave the dashboard.
    Leave,
}

/// Where an action stands.
///
/// Every one of these is a state a frame can be drawn in. Being asked what a stack
/// can be acted on is not one: [`Wanted::Ask`] and [`Acting::told`] are one statement
/// in the loop, over a read that does not await, so no frame is drawn between them —
/// which is why the offer waiting for that answer is a field rather than a stage.
enum Stage {
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
}

/// What this screen has open, and what it is waiting for.
pub(crate) struct Acting {
    /// Where the action stands.
    stage: Stage,
    /// The action an outstanding [`Wanted::Ask`] was begun for.
    ///
    /// Taken by [`Acting::told`], so an answer to a question that was never asked —
    /// or was asked for something else — changes nothing.
    asked: Option<&'static Offer>,
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
            Stage::Choosing { offer, chooser } => self.choosing(offer, chooser, press),
            Stage::Confirming { offer, chosen } => self.confirming(offer, chosen, press),
            Stage::Running { offer, chosen } => self.while_running(offer, chosen, press),
            Stage::Wondering(chooser) => self.wondering(chooser, press),
            Stage::Typing {
                question,
                asks,
                typed,
            } => self.typing(question, asks, typed, press),
            Stage::Waiting(question) => self.while_waiting(question, press),
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
            Press::Typed(key) => self.begin(key),
            Press::Rubout | Press::Back | Press::Forward | Press::Accept => Wanted::Nothing,
        }
    }

    /// Begin the action a key reaches, by asking what there is to act on.
    ///
    /// The list is asked for rather than remembered from a previous run: a stack's
    /// declarations are a file on disk that an operator may have just edited, and a
    /// list gathered once would offer a form that is no longer there.
    fn begin(&mut self, key: char) -> Wanted {
        let Some(offer) = offer::for_key(key) else {
            return Wanted::Nothing;
        };
        self.asked = Some(offer);
        Wanted::Ask(Command::Forms)
    }

    /// Over the list: move, take one, or leave it.
    fn choosing(
        &mut self,
        offer: &'static Offer,
        mut chooser: Chooser<Choice>,
        press: &Press,
    ) -> Wanted {
        match *press {
            Press::Abandon => return Wanted::Nothing,
            Press::Accept => {
                self.stage = Stage::Confirming {
                    offer,
                    chosen: chooser.taken(),
                };
                return Wanted::Nothing;
            }
            Press::Back => chooser.back(),
            Press::Forward => chooser.forward(),
            Press::Typed(_) | Press::Rubout => (),
        }
        self.stage = Stage::Choosing { offer, chooser };
        Wanted::Nothing
    }

    /// At the question: only an explicit yes goes ahead.
    ///
    /// Everything else — a no, a stray return, a key that is neither — leaves the
    /// stack as it is, which is the same way the teardown's own question is read.
    /// The answer that changes something should never be the one given by accident.
    fn confirming(&mut self, offer: &'static Offer, chosen: Choice, press: &Press) -> Wanted {
        if !matches!(*press, Press::Typed('y' | 'Y')) {
            return Wanted::Nothing;
        }
        let command = chosen.command.clone();
        self.stage = Stage::Running { offer, chosen };
        Wanted::Carry(command)
    }

    /// While the action is with the core: leaving is the only thing left to ask.
    ///
    /// The stage is put back either way. Leaving does not stop the action — the run
    /// waits for it once the screen is given back — so it is still where it was, and
    /// [`Acting::staying_for`] is what says so on the way out.
    fn while_running(&mut self, offer: &'static Offer, chosen: Choice, press: &Press) -> Wanted {
        self.stage = Stage::Running { offer, chosen };
        if matches!(*press, Press::Typed('q') | Press::Abandon) {
            return Wanted::Leave;
        }
        Wanted::Nothing
    }

    /// Over the questions: move, take one, or leave it.
    fn wondering(&mut self, mut chooser: Chooser<&'static Question>, press: &Press) -> Wanted {
        match *press {
            Press::Abandon => return Wanted::Nothing,
            Press::Accept => return self.take(chooser.taken()),
            Press::Back => chooser.back(),
            Press::Forward => chooser.forward(),
            Press::Typed(_) | Press::Rubout => (),
        }
        self.stage = Stage::Wondering(chooser);
        Wanted::Nothing
    }

    /// Ask the question that was taken, or open the line it has to be given first.
    fn take(&mut self, question: &'static Question) -> Wanted {
        match question.needs.asks() {
            Some(asks) => {
                self.stage = Stage::Typing {
                    question,
                    asks,
                    typed: String::new(),
                };
                Wanted::Nothing
            }
            None => self.put(question, ""),
        }
    }

    /// Over the line being typed: type, take back, ask, or leave it.
    fn typing(
        &mut self,
        question: &'static Question,
        asks: &'static str,
        mut typed: String,
        press: &Press,
    ) -> Wanted {
        match *press {
            Press::Abandon => return Wanted::Nothing,
            Press::Accept => return self.put(question, &typed),
            Press::Rubout => {
                typed.pop();
            }
            Press::Typed(character) => typed.push(character),
            Press::Back | Press::Forward => (),
        }
        self.stage = Stage::Typing {
            question,
            asks,
            typed,
        };
        Wanted::Nothing
    }

    /// Put the question to the core, or say why it cannot be put.
    ///
    /// Carried rather than awaited, because a question about what the household
    /// asked for reaches the services over the network and a screen that waited on
    /// it would stop answering keys while it did.
    fn put(&mut self, question: &'static Question, typed: &str) -> Wanted {
        match question.command(typed) {
            Ok(command) => {
                self.stage = Stage::Waiting(question);
                Wanted::Carry(command)
            }
            Err(said) => {
                self.stage = Stage::Answered {
                    question,
                    reading: Reading::of(vec![said.to_owned()]),
                };
                Wanted::Nothing
            }
        }
    }

    /// While the question is with the core: back out, or wait for it.
    fn while_waiting(&mut self, question: &'static Question, press: &Press) -> Wanted {
        if matches!(*press, Press::Abandon) {
            return Wanted::Nothing;
        }
        self.stage = Stage::Waiting(question);
        Wanted::Nothing
    }

    /// What the stack answered when it was asked what there is to act on.
    pub(crate) fn told(&mut self, answer: Result<Outcome, Box<Problem>>) {
        let Some(offer) = self.asked.take() else {
            return;
        };
        self.stage = match answer {
            Ok(Outcome::Forms(report)) => match offer.given(&report) {
                Ok((selected, rest)) => Stage::Choosing {
                    offer,
                    chooser: Chooser::over(selected, rest),
                },
                Err(refused) => Stage::Came(Reading::of(vec![refused])),
            },
            Ok(_) => Stage::Came(Reading::of(unexpected())),
            Err(problem) => Stage::Came(Reading::of(complaint(&problem))),
        };
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

/// Move through a reading, saying whether the press was a move at all.
///
/// A press that is not a move puts the reading away, which is how the pane of words
/// is put away too: one shape of dismissal on a screen where several boxes open.
fn moved(reading: &mut Reading, press: &Press) -> bool {
    match *press {
        Press::Back => reading.back(),
        Press::Forward => reading.forward(),
        _ => return false,
    }
    true
}

/// An answer that is not the shape the question had.
///
/// Every command this screen sends has one shape of answer, so nothing reaches
/// this in an ordinary run. It is said rather than shown as nothing, because a
/// screen that went quiet would read as an action that never ran.
fn unexpected() -> Vec<String> {
    vec![NOT_THE_ANSWER.to_owned()]
}

/// A failure, in the words the command line gives for the same one.
fn complaint(problem: &Problem) -> Vec<String> {
    lines_of(&crate::exit::reported(problem, false))
}

/// A rendered answer as the rows a screen draws.
fn lines_of(lines: &crate::render::Lines) -> Vec<String> {
    lines.text().lines().map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::offer::tests::{a_listing, nothing_declared};
    use super::{meaning, question, Acting, Line, Press, Wanted};
    use crate::render::fixtures::{a_lifecycle, a_plan};
    use lemonfiber_core::app::{Command, Outcome};
    use lemonfiber_core::error::{Code, Problem, Remedy, Severity};
    use lemonfiber_core::model::{FormsReport, VersionReport};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
        assert!(said.contains("> Full stack"), "{said}");

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
                wait: false
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
}
