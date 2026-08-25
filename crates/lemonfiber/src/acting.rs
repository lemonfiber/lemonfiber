//! What a keypress on the dashboard asks for, and what becomes of it.
//!
//! The dashboard read and offered nothing to do about what it read. This is the
//! deciding half of doing something: which key reaches which action, what an action
//! may be given, the question put before it, and what is said about what it came
//! to. None of it is in [`crate::terminal`], which is a real terminal in raw mode
//! and the one file this workspace deliberately does not test — a decision behind
//! that filename is a decision nothing checks.
//!
//! **An action reaches the command every other surface reaches.** The name is put
//! through the web surface's own translation rather than assembled here, so this
//! screen cannot grow an action a browser has no form of. That is the whole point
//! of the arrangement: a terminal action that did something no other surface could
//! do would defeat the requirement it was built for.
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
//! Leaving stops this screen waiting; what the container engine was already asked
//! to do is between the operator and the engine, exactly as a closed browser tab
//! takes nothing with it.

mod chooser;
mod offer;
mod words;

use lemonfiber_core::app::{Command, Outcome};
use lemonfiber_core::error::Problem;
use ratatui::text::Line;

use chooser::Chooser;
use offer::{Choice, Offer};

pub(crate) use words::Pane;

/// What is said where an answer is not the shape the question had.
const NOT_THE_ANSWER: &str = "This stack answered something other than what was asked of it.";

/// What the operator pressed.
pub(crate) enum Press {
    /// A character.
    Typed(char),
    /// The entry above the one selected.
    Back,
    /// The entry below it.
    Forward,
    /// Take what is selected.
    Accept,
    /// Back out of whatever is open.
    Abandon,
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
enum Stage {
    /// Nothing is open.
    Idle,
    /// Waiting to be told what this stack can be asked to act on.
    Asking(&'static Offer),
    /// Choosing what to act on.
    Choosing {
        /// The action being chosen for.
        offer: &'static Offer,
        /// What it can be given, one of them selected.
        chooser: Chooser,
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
    Came(Vec<String>),
}

/// What this screen has open, and what it is waiting for.
pub(crate) struct Acting {
    /// Where the action stands.
    stage: Stage,
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
            Stage::Asking(offer) => self.waiting(offer, press),
            Stage::Choosing { offer, chooser } => self.choosing(offer, chooser, press),
            Stage::Confirming { offer, chosen } => self.confirming(offer, chosen, press),
            Stage::Running { offer, chosen } => self.while_running(offer, chosen, press),
            // A report is put away by any key, the way the words are.
            Stage::Came(_) => Wanted::Nothing,
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
            Press::Typed(key) => self.begin(key),
            Press::Back | Press::Forward | Press::Accept => Wanted::Nothing,
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
        self.stage = Stage::Asking(offer);
        Wanted::Ask(Command::Forms)
    }

    /// While the stack is being asked: back out, or wait for it.
    fn waiting(&mut self, offer: &'static Offer, press: &Press) -> Wanted {
        if matches!(*press, Press::Abandon) {
            return Wanted::Nothing;
        }
        self.stage = Stage::Asking(offer);
        Wanted::Nothing
    }

    /// Over the list: move, take one, or leave it.
    fn choosing(&mut self, offer: &'static Offer, mut chooser: Chooser, press: &Press) -> Wanted {
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
            Press::Typed(_) => (),
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
    fn while_running(&mut self, offer: &'static Offer, chosen: Choice, press: &Press) -> Wanted {
        if matches!(*press, Press::Typed('q') | Press::Abandon) {
            return Wanted::Leave;
        }
        self.stage = Stage::Running { offer, chosen };
        Wanted::Nothing
    }

    /// What the stack answered when it was asked what there is to act on.
    pub(crate) fn told(&mut self, answer: Result<Outcome, Box<Problem>>) {
        let offer = match &self.stage {
            Stage::Asking(offer) => *offer,
            _ => return,
        };
        self.stage = match answer {
            Ok(Outcome::Forms(report)) => match offer.given(&report) {
                Ok((selected, rest)) => Stage::Choosing {
                    offer,
                    chooser: Chooser::over(selected, rest),
                },
                Err(refused) => Stage::Came(vec![refused]),
            },
            Ok(_) => Stage::Came(unexpected()),
            Err(problem) => Stage::Came(complaint(&problem)),
        };
    }

    /// What the action came to.
    pub(crate) fn came_to(&mut self, answer: Result<Outcome, Box<Problem>>) {
        self.stage = Stage::Came(match answer {
            Ok(Outcome::Lifecycle(report)) => lines_of(&crate::render::stack::lifecycle(&report)),
            Ok(_) => unexpected(),
            Err(problem) => complaint(&problem),
        });
    }

    /// The box over the screen, or nothing where the action has none open.
    pub(crate) fn pane(&self, rows: usize, across: usize) -> Option<Pane> {
        words::pane(&self.stage, rows, across)
    }

    /// The one line at the foot of the screen.
    pub(crate) fn footer(&self, across: usize) -> Line<'static> {
        words::footer(&self.stage, across)
    }
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
    use super::{Acting, Line, Press, Wanted};
    use crate::render::fixtures::{a_lifecycle, a_plan};
    use lemonfiber_core::app::{Command, Outcome};
    use lemonfiber_core::error::{Code, Problem, Remedy, Severity};
    use lemonfiber_core::model::{FormsReport, VersionReport};

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

    /// An answer of a shape this screen never asks for.
    fn a_version() -> VersionReport {
        VersionReport {
            binary: String::new(),
            supported_schema: Vec::new(),
            stack: String::new(),
            compose: None,
        }
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

        assert_eq!(carried, Wanted::Carry(Command::Down { forms: Vec::new() }));
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

    /// Backing out while the stack is being asked does the same, and anything else
    /// pressed there leaves the question standing.
    #[test]
    fn a_screen_waiting_on_the_stack_can_be_left_and_otherwise_waits() {
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed('u'));
        assert!(showing(&acting).contains("asking this stack"));

        assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
        assert!(showing(&acting).contains("asking this stack"));

        assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
        assert!(showing(&acting).is_empty());
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

    /// Leaving while an action runs is allowed and stops nothing but the watching.
    /// Anything else pressed leaves it running.
    #[test]
    fn leaving_while_an_action_runs_stops_the_watching_and_not_the_work() {
        let (mut acting, _) = choosing('t', a_listing());
        acting.pressed(&Press::Accept);
        acting.pressed(&Press::Typed('y'));

        assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
        assert!(footing(&acting).contains("still running"));

        assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);
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

        let mut acted = Acting::opened();
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

    /// Every key an action is on begins that action, and every one of them reaches
    /// the same list of what it can be given.
    #[test]
    fn every_key_an_action_is_on_begins_that_action() {
        for key in ['u', 'd', 's', 't', 'p'] {
            let mut acting = Acting::opened();

            let wanted = acting.pressed(&Press::Typed(key));

            assert_eq!(wanted, Wanted::Ask(Command::Forms), "{key}");
            assert!(showing(&acting).contains("asking this stack"), "{key}");
        }
    }
}
