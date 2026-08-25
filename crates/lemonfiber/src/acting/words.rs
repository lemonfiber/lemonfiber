//! What an action puts on the screen, as lines.
//!
//! Lines rather than widgets, for the reason the panels beside them are: what the
//! screen *says* is the part worth proving, and where the box goes is not. Every
//! one of these is a pure function over where the action stands, so the words are
//! read back without a terminal anywhere near them.
//!
//! Nothing here asks for a colour. Severity is not what this screen carries — a
//! question, a list and a report are none of them warnings — so what marks the
//! secondary line is dimming, which is an attribute rather than a colour and
//! survives a terminal that has been told to use none.
//!
//! Text from somewhere else — a form's own name, another service's account of what
//! went wrong — reaches the screen through [`shortened`] and never past it, so no
//! line can be put up by a route that skips being made safe for a terminal.

use lemonfiber_core::plural::s;
use lemonfiber_core::text::{fitted, plain};
use ratatui::text::{Line, Span};

use super::chooser::{Chooser, Listed};
use super::errand::{self, Errand};
use super::offer::{Choice, Offer, OFFERED};
use super::question::{self, Question};
use super::reading::Reading;
use super::Stage;
use crate::pane::quiet;

/// The keys the screen answers whatever else is open.
const ALWAYS: &str = "q quit   r refresh   ? words";

/// What the box holding a report calls itself.
const CAME: &str = " what it came to ";

/// What the box holding the questions calls itself.
const ASK: &str = " ask ";

/// What the box holding the rest of the errands calls itself.
const MORE: &str = " more ";

/// What the box says while the core is working out what an errand would do.
const WEIGHING: &str = "working out what this would do";

/// What the box says while a question is with the stack.
const ANSWERING: &str = "waiting for this stack to answer";

/// How a word a question has to be given is typed, and how to leave it.
const TYPING: &str = "enter asks   esc leaves it";

/// How a reading is moved through, and how it is put away.
const MOVING: &str = "up and down move   any other key closes";

/// How to move over the list, and how to leave it.
const CHOOSING: &str = "up and down choose   enter goes on   esc leaves it";

/// How the question before an action is answered.
///
/// Only an explicit yes goes ahead, which is how the teardown's own question is
/// read too: an answer that is neither should land on the thing that changes
/// nothing, and on a screen where one key reaches an action it is a keypress that
/// was not meant that this is guarding against.
const AGREEING: &str = "y goes ahead   any other key changes nothing";

/// The same, under an answer the box also moves through.
const READING_AND_AGREEING: &str =
    "up and down move   y goes ahead   any other key changes nothing";

/// What is said beside a running action, and what leaving does about it.
///
/// There is no daemon behind this screen. The process drawing it is the one that
/// claimed the stack and issued the command, so leaving is not a tab being closed
/// on a server that carries on: the screen goes at once, and the run stays until
/// the action it started has finished and given the stack back.
const WAITING: &str = "still running   q closes the screen and waits for it";

/// A box over the screen: what it is called, and what it says.
pub(crate) struct Pane {
    /// What the box is called, on its own border.
    pub(crate) title: String,
    /// What it says.
    pub(crate) lines: Vec<Line<'static>>,
}

/// The box over the screen, or nothing where an action has none open.
///
/// A running action has none, and neither has an errand under way. The screen behind
/// them is the report — the panels go on gathering every second while the work runs,
/// so what the action is doing to the services shows where the services are listed,
/// and a box over that would take away the one thing worth watching.
pub(super) fn pane(stage: &Stage, rows: usize, across: usize) -> Option<Pane> {
    match stage {
        Stage::Idle | Stage::Running { .. } | Stage::Doing { .. } => None,
        Stage::Choosing { offer, chooser } => Some(Pane {
            title: titled(offer),
            lines: choosing(chooser, rows, across),
        }),
        Stage::Confirming { offer, chosen } => Some(Pane {
            title: titled(offer),
            lines: confirming(offer, chosen, across),
        }),
        Stage::Came(reading) => Some(Pane {
            title: CAME.to_owned(),
            lines: read(reading, rows, across),
        }),
        Stage::Wondering(chooser) => Some(Pane {
            title: ASK.to_owned(),
            lines: choosing(chooser, rows, across),
        }),
        Stage::Typing { asks, typed, .. } => Some(Pane {
            title: ASK.to_owned(),
            lines: typing(asks, typed, across),
        }),
        Stage::Waiting(question) => Some(Pane {
            title: asked(question),
            lines: vec![dimmed(ANSWERING, across)],
        }),
        Stage::Answered { question, reading } => Some(Pane {
            title: asked(question),
            lines: read(reading, rows, across),
        }),
        Stage::Sending(chooser) => Some(Pane {
            title: MORE.to_owned(),
            lines: choosing(chooser, rows, across),
        }),
        Stage::Naming {
            errand,
            asks,
            typed,
        } => Some(Pane {
            title: sending(errand),
            lines: typing(asks, typed, across),
        }),
        Stage::Weighing { errand, .. } => Some(Pane {
            title: sending(errand),
            lines: vec![dimmed(WEIGHING, across)],
        }),
        Stage::Agreeing {
            errand,
            typed,
            would,
        } => Some(Pane {
            title: sending(errand),
            lines: agreeing(errand, typed, would.as_ref(), rows, across),
        }),
    }
}

/// The one line at the foot of the screen: the keys, or what is running.
///
/// The keys give way to the running action rather than sitting beside it. There is
/// one row, an operator who has just started something is owed what it is more than
/// they are owed a list they have just used, and the keys come back the moment it
/// is over.
pub(super) fn footer(stage: &Stage, across: usize) -> Line<'static> {
    let said = match stage {
        Stage::Running { offer, chosen } => {
            format!("{} {}   {WAITING}", offer.hint, chosen.name)
        }
        Stage::Doing { errand, .. } => format!("{}   {WAITING}", errand.name),
        _ => keys(),
    };
    dimmed(&said, across)
}

/// What a run leaving this screen now would stay for, or nothing where it may go.
///
/// Only an action. A read is with the core the same way and claims nothing, so a
/// screen left with one outstanding has nothing to stay for — which is why this asks
/// about [`Stage::Running`] and not about [`Stage::Waiting`].
///
/// Said on the ordinary terminal once the screen is given back, where there is room
/// for the whole of it and no width to fit — so this is the one line here that goes
/// through [`plain`] alone rather than through [`shortened`].
pub(super) fn staying_for(stage: &Stage) -> Option<String> {
    let doing = match stage {
        Stage::Running { offer, chosen } => format!("{} {}", offer.hint, chosen.name),
        Stage::Doing { errand, .. } => errand.name.to_owned(),
        _ => return None,
    };
    Some(plain(&format!(
        "waiting for {doing} to finish — leaving it now would leave the stack claimed"
    )))
}

/// Every key this screen answers, in the order they are worth reading.
///
/// Built from the offers rather than written out, so an action added to the table
/// is an action the operator is told about.
fn keys() -> String {
    let mut said = vec![
        ALWAYS.to_owned(),
        format!("{} {}", question::KEY, question::HINT),
        format!("{} {}", errand::KEY, errand::HINT),
    ];
    said.extend(
        OFFERED
            .iter()
            .map(|offer| format!("{} {}", offer.key, offer.hint)),
    );
    said.join("   ")
}

/// What the box holding one action is called.
fn titled(offer: &Offer) -> String {
    format!(" {} ", offer.hint)
}

/// What the box holding one question's answer is called.
fn asked(question: &Question) -> String {
    format!(" {} ", question.name)
}

/// What the box holding one errand is called.
fn sending(errand: &Errand) -> String {
    format!(" {} ", errand.name)
}

/// The entries, the selected one marked, and how to move over them.
fn choosing<T: Listed>(chooser: &Chooser<T>, rows: usize, across: usize) -> Vec<Line<'static>> {
    // Two rows are kept back for the blank and the hint under the list, which is
    // what tells an operator that enter is what they are looking for.
    let room = rows.saturating_sub(2);
    let mut lines: Vec<Line<'static>> = chooser
        .listed()
        .take(room)
        .map(|(here, choice)| offered(here, choice, across))
        .collect();
    let left = chooser.listed().count().saturating_sub(lines.len());
    if left > 0 {
        lines.push(dimmed(
            &format!(
                "{left} more choice{} than this screen has room for",
                s(left)
            ),
            across,
        ));
    }
    lines.push(Line::raw(""));
    lines.push(dimmed(CHOOSING, across));
    lines
}

/// One entry: its name, and what it is for beside it.
fn offered(here: bool, entry: &impl Listed, across: usize) -> Line<'static> {
    let mark = if here { "> " } else { "  " };
    // The marker and the two spaces after the name are taken off before the name is
    // fitted, so the row it ends up on is the width it was given rather than that
    // width plus whatever the marker cost.
    let named = format!(
        "{mark}{}  ",
        shortened(entry.name(), across.saturating_sub(4))
    );
    let room = across.saturating_sub(named.chars().count());
    Line::from(vec![
        Span::raw(named),
        Span::styled(shortened(entry.about(), room), quiet()),
    ])
}

/// What a question has to be given, and what has been typed of it so far.
///
/// The word is drawn as typed and never made to fit from the left, so what is being
/// typed stays where it was put — a field that scrolled under the operator's own
/// fingers would be a field nobody could correct.
fn typing(asks: &str, typed: &str, across: usize) -> Vec<Line<'static>> {
    vec![
        Line::raw(shortened(asks, across)),
        Line::raw(shortened(&format!("> {typed}"), across)),
        Line::raw(""),
        dimmed(TYPING, across),
    ]
}

/// The question before an errand, under what it would do where the errand could say.
///
/// Under it rather than over it: an effect somebody reads after they have agreed is
/// not something they agreed to, so what a reset would revert and what a bundle would
/// hold are the lines above the question rather than the answer to it.
fn agreeing(
    errand: &Errand,
    typed: &str,
    would: Option<&Reading>,
    rows: usize,
    across: usize,
) -> Vec<Line<'static>> {
    let asks = format!("{} {typed}", errand.asks);
    let mut lines: Vec<Line<'static>> = Vec::new();
    if let Some(would) = would {
        // Three rows are kept back for the question, the blank and the hint under
        // it, so what it would do never grows over the thing being agreed to.
        let (shown, above, below) = would.window(rows.saturating_sub(4));
        lines.extend(
            shown
                .into_iter()
                .map(|line| Line::raw(shortened(line, across))),
        );
        if let Some(place) = elsewhere(above, below) {
            lines.push(dimmed(&place, across));
        }
    }
    lines.push(Line::raw(shortened(
        &format!("{}?", asks.trim_end()),
        across,
    )));
    lines.push(dimmed(errand.about, across));
    lines.push(Line::raw(""));
    lines.push(dimmed(
        if would.is_some() {
            READING_AND_AGREEING
        } else {
            AGREEING
        },
        across,
    ));
    lines
}

/// The question before an action, and what it is being asked about.
fn confirming(offer: &Offer, chosen: &Choice, across: usize) -> Vec<Line<'static>> {
    vec![
        Line::raw(shortened(
            &format!("{} {}?", offer.asks, chosen.name),
            across,
        )),
        dimmed(&chosen.about, across),
        Line::raw(""),
        dimmed(AGREEING, across),
    ]
}

/// An answer, in the words the command line gives for the same question.
///
/// What is not on the screen is counted rather than left to be inferred from a box
/// that has stopped moving, because either end of a reading looks the same as a
/// reading that was short.
fn read(reading: &Reading, rows: usize, across: usize) -> Vec<Line<'static>> {
    // Two rows are kept back for the blank and the hint under the answer, which is
    // what tells an operator the box moves at all.
    let room = rows.saturating_sub(2);
    let (shown, above, below) = reading.window(room);
    let mut lines: Vec<Line<'static>> = shown
        .into_iter()
        .map(|line| Line::raw(shortened(line, across)))
        .collect();
    if let Some(place) = elsewhere(above, below) {
        lines.push(dimmed(&place, across));
    }
    lines.push(Line::raw(""));
    lines.push(dimmed(MOVING, across));
    lines
}

/// What is off the top and off the bottom of the box, or nothing where it holds
/// the whole answer.
fn elsewhere(above: usize, below: usize) -> Option<String> {
    let over = format!("{above} more line{} above", s(above));
    let under = format!("{below} more line{} below", s(below));
    match (above, below) {
        (0, 0) => None,
        (0, _) => Some(under),
        (_, 0) => Some(over),
        _ => Some(format!("{over}, {under}")),
    }
}

/// A line that is not the one being read, drawn as such.
fn dimmed(said: &str, across: usize) -> Line<'static> {
    Line::styled(shortened(said, across), quiet())
}

/// Text made safe for a terminal and then made to fit the row it has.
///
/// One place, so no line can be put on the screen by a route that skips either
/// half of it — the same rule the panels behind this one are held to.
fn shortened(value: &str, room: usize) -> String {
    fitted(&plain(value), room)
}

#[cfg(test)]
mod tests {
    use super::{elsewhere, footer, keys, pane, read, staying_for, Offer, Stage};
    use crate::acting::chooser::Chooser;
    use crate::acting::errand::{self, Errand};
    use crate::acting::offer::{Choice, OFFERED};
    use crate::acting::question::{Needed, Question, HINT, KEY};
    use crate::acting::reading::Reading;
    use lemonfiber_core::app::Command;
    use ratatui::text::Line;

    /// One action, held here rather than taken out of the table by number, since
    /// what these read is what is drawn and not which action it was.
    static A_START: Offer = Offer {
        key: 'u',
        action: "up",
        hint: "start",
        asks: "Start",
    };

    /// One question that takes a word, held here for the same reason.
    static A_TRACE: Question = Question {
        name: "where one thing is",
        about: "follow one show or film across the services",
        read: "/api/trace",
        needs: Needed::Term("What to follow"),
    };

    /// A reading over nine numbered lines.
    fn nine() -> Reading {
        Reading::of((0..9).map(|at| format!("line {at}")).collect())
    }

    /// A choice by name, for the tests that only read what is drawn.
    fn a_choice(name: &str, about: &str) -> Choice {
        Choice {
            name: name.to_owned(),
            about: about.to_owned(),
            command: Command::Up {
                forms: vec![name.to_owned()],
            },
        }
    }

    /// One line as text, its spans joined the way the screen shows them.
    fn text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<Vec<&str>>()
            .concat()
    }

    /// Everything a pane says, as one piece of text.
    fn said(stage: &Stage, rows: usize, across: usize) -> String {
        pane(stage, rows, across).map_or_else(String::new, |drawn| {
            let mut all = vec![drawn.title.clone()];
            all.extend(drawn.lines.iter().map(text));
            all.join("\n")
        })
    }

    /// The errand one action is on, taken from the list the screen really offers
    /// rather than built here — what these read is the words an operator gets.
    fn sent(action: &str) -> &'static Errand {
        let (first, rest) = errand::all();
        std::iter::once(first)
            .chain(rest)
            .find(|errand| errand.action == action)
            .unwrap_or(first)
    }

    /// A chooser over two, for the list tests.
    fn two() -> Chooser<Choice> {
        Chooser::over(
            a_choice("Full stack", "everything, behind the tunnel"),
            vec![a_choice("Lean stack", "the download clients only")],
        )
    }

    /// Every action is on the footer, or the operator has no way to learn a key
    /// exists — a screen whose only account of what it can do is its source.
    #[test]
    fn the_footer_names_every_key_this_screen_answers() {
        let said = keys();

        assert!(said.contains("q quit"), "{said}");
        assert!(said.contains(&format!("{KEY} {HINT}")), "{said}");
        assert!(
            said.contains(&format!("{} {}", errand::KEY, errand::HINT)),
            "{said}"
        );
        for offer in OFFERED {
            assert!(
                said.contains(offer.hint),
                "{} is missing: {said}",
                offer.hint
            );
            assert!(said.contains(offer.key), "{} is missing: {said}", offer.key);
        }
    }

    /// While something is running the footer says so, and says what leaving does —
    /// which is the one thing an operator watching a teardown needs to know. What it
    /// must not say is that the work outlives the screen: the process drawing it is
    /// the one carrying the work out.
    #[test]
    fn a_running_action_says_what_leaving_the_screen_does_about_it() {
        let stage = Stage::Running {
            offer: &A_START,
            chosen: a_choice("Full stack", "everything"),
        };

        let said = text(&footer(&stage, 120));

        assert!(said.contains("Full stack"), "{said}");
        assert!(said.contains("still running"), "{said}");
        assert!(said.contains("closes the screen and waits"), "{said}");
        assert!(!said.contains("the work goes on"), "{said}");
    }

    /// Leaving mid-action says which action is being waited on and why, because an
    /// operator who pressed q and got a wait instead of a shell is owed both.
    #[test]
    fn leaving_mid_action_says_what_is_being_waited_on_and_why() {
        let stage = Stage::Running {
            offer: &A_START,
            chosen: a_choice("Full stack", "everything"),
        };

        let said = staying_for(&stage).unwrap_or_default();

        assert!(said.contains("start Full stack"), "{said}");
        assert!(said.contains("leave the stack claimed"), "{said}");
        assert!(
            staying_for(&Stage::Idle).is_none(),
            "nothing runs, nothing said"
        );
    }

    /// A form's own name reaches an ordinary terminal here rather than a drawn row,
    /// and a control character is an instruction to both.
    #[test]
    fn a_name_from_somewhere_else_is_made_safe_before_it_is_said() {
        let stage = Stage::Running {
            offer: &A_START,
            chosen: a_choice("Full\u{1b}[2Jstack", "everything"),
        };

        let said = staying_for(&stage).unwrap_or_default();

        assert!(!said.contains('\u{1b}'), "{said:?}");
        assert!(said.contains("Full[2Jstack"), "{said}");
    }

    /// A running action draws no box, because the panels behind it are the report.
    #[test]
    fn a_running_action_leaves_the_screen_behind_it_visible() {
        let stage = Stage::Running {
            offer: &A_START,
            chosen: a_choice("Full stack", "everything"),
        };

        assert!(pane(&stage, 20, 80).is_none());
        assert!(pane(&Stage::Idle, 20, 80).is_none());
        assert!(text(&footer(&Stage::Idle, 120)).contains("r refresh"));
    }

    /// The list says what each choice is for, marks the one selected, and says how
    /// to take it.
    #[test]
    fn the_list_marks_what_is_selected_and_says_how_to_take_it() {
        let stage = Stage::Choosing {
            offer: &A_START,
            chooser: two(),
        };

        let said = said(&stage, 20, 80);

        assert!(said.contains("> Full stack"), "{said}");
        assert!(said.contains("  Lean stack"), "{said}");
        assert!(said.contains("the download clients only"), "{said}");
        assert!(said.contains("enter goes on"), "{said}");
    }

    /// A list longer than the screen counts what it left out rather than showing
    /// one of two as though there were one.
    #[test]
    fn a_list_too_long_for_the_screen_counts_what_it_left_out() {
        let stage = Stage::Choosing {
            offer: &A_START,
            chooser: two(),
        };

        let short = said(&stage, 3, 80);
        let whole = said(&stage, 20, 80);

        assert!(short.contains("1 more choice"), "{short}");
        assert!(!whole.contains("more choice"), "{whole}");
    }

    /// The question names the action and what it is about, and says which way
    /// saying nothing falls.
    #[test]
    fn the_question_names_what_is_about_to_happen_and_to_what() {
        let stage = Stage::Confirming {
            offer: &A_START,
            chosen: a_choice("Full stack", "everything, behind the tunnel"),
        };

        let said = said(&stage, 20, 80);

        assert!(said.contains("Start Full stack?"), "{said}");
        assert!(said.contains("everything, behind the tunnel"), "{said}");
        assert!(said.contains("any other key changes nothing"), "{said}");
    }

    /// An answer says how to move through it, and counts what is off each end —
    /// because either end of a reading looks exactly like a reading that was short.
    #[test]
    fn an_answer_longer_than_the_screen_counts_what_is_off_each_end() {
        let mut reading = nine();

        let opened: Vec<String> = read(&reading, 4, 80).iter().map(text).collect();
        reading.forward();
        let moved: Vec<String> = read(&reading, 4, 80).iter().map(text).collect();
        let whole: Vec<String> = read(&nine(), 40, 80).iter().map(text).collect();

        assert!(
            opened
                .iter()
                .any(|line| line.contains("7 more lines below")),
            "{opened:?}"
        );
        assert!(
            moved
                .iter()
                .any(|line| line.contains("1 more line above, 6 more lines below")),
            "{moved:?}"
        );
        assert!(
            whole.iter().any(|line| line.contains("line 8")),
            "{whole:?}"
        );
        assert!(
            !whole.iter().any(|line| line.contains("more line")),
            "{whole:?}"
        );
        assert!(
            whole.iter().any(|line| line.contains("up and down move")),
            "{whole:?}"
        );
    }

    /// The end of a long answer says what is behind it and claims nothing is ahead.
    #[test]
    fn the_end_of_an_answer_says_only_what_is_behind_it() {
        assert_eq!(elsewhere(0, 0), None);
        assert_eq!(elsewhere(8, 0), Some("8 more lines above".to_owned()));
        assert_eq!(elsewhere(1, 0), Some("1 more line above".to_owned()));
    }

    /// The list of questions is the list of choices, drawn by the same thing: a
    /// second way to draw a list is a second way for two lists to disagree.
    #[test]
    fn the_questions_are_listed_the_way_the_choices_are() {
        let stage = Stage::Wondering(Chooser::over(&A_TRACE, Vec::new()));

        let said = said(&stage, 20, 80);

        assert!(said.contains("> where one thing is"), "{said}");
        assert!(said.contains("follow one show or film"), "{said}");
        assert!(said.contains("enter goes on"), "{said}");
    }

    /// A question that takes a word says what it wants, shows what has been typed,
    /// and says which key asks it.
    #[test]
    fn a_question_taking_a_word_says_what_it_wants_and_what_was_typed() {
        let stage = Stage::Typing {
            question: &A_TRACE,
            asks: "What to follow",
            typed: "The Exp".to_owned(),
        };

        let said = said(&stage, 20, 80);

        assert!(said.contains("What to follow"), "{said}");
        assert!(said.contains("> The Exp"), "{said}");
        assert!(said.contains("enter asks"), "{said}");
    }

    /// A question with the core says so, under the name of the question asked, so
    /// an answer that is slow to arrive does not read as a screen that stopped.
    #[test]
    fn a_question_with_the_core_says_what_it_is_waiting_for() {
        let said = said(&Stage::Waiting(&A_TRACE), 20, 80);

        assert!(said.contains("where one thing is"), "{said}");
        assert!(said.contains("waiting for this stack to answer"), "{said}");
    }

    /// An answer is drawn under the name of the question it answers, so a box that
    /// is still open a minute later still says what it was asked.
    #[test]
    fn an_answer_is_drawn_under_the_question_it_answers() {
        let stage = Stage::Answered {
            question: &A_TRACE,
            reading: nine(),
        };

        let said = said(&stage, 20, 80);

        assert!(said.contains("where one thing is"), "{said}");
        assert!(said.contains("line 0"), "{said}");
    }

    /// A control character in another service's account of a failure is an
    /// instruction to a terminal, and the screen must not carry one.
    #[test]
    fn text_from_somewhere_else_is_made_safe_before_it_is_drawn() {
        let stage = Stage::Confirming {
            offer: &A_START,
            chosen: a_choice("Full\u{1b}[2Jstack", "quietly clearing the screen"),
        };

        let said = said(&stage, 20, 120);

        assert!(!said.contains('\u{1b}'), "{said:?}");
        assert!(said.contains("Full[2Jstack"), "{said}");
    }

    /// A narrow screen shortens rather than running past its edge, whichever of the
    /// boxes is open on it.
    #[test]
    fn no_row_runs_past_the_edge_of_a_narrow_screen() {
        let opened = [
            Stage::Choosing {
                offer: &A_START,
                chooser: two(),
            },
            Stage::Confirming {
                offer: &A_START,
                chosen: a_choice("Full stack", "everything, behind the tunnel"),
            },
            Stage::Wondering(Chooser::over(&A_TRACE, Vec::new())),
            Stage::Typing {
                question: &A_TRACE,
                asks: "What to follow",
                typed: "something with a very long name indeed".to_owned(),
            },
            Stage::Waiting(&A_TRACE),
            Stage::Answered {
                question: &A_TRACE,
                reading: nine(),
            },
            Stage::Came(nine()),
        ];

        for stage in &opened {
            for across in [4usize, 12, 24, 40] {
                for row in said(stage, 20, across).lines().skip(1) {
                    assert!(row.chars().count() <= across, "at {across}: {row:?}");
                }
            }
        }
    }

    /// The rest of the errands are listed the way every other list is, so what an
    /// operator learned on one box carries to the next.
    #[test]
    fn the_errands_are_listed_the_way_the_choices_are() {
        let (first, rest) = errand::all();

        let said = said(&Stage::Sending(Chooser::over(first, rest)), 20, 90);

        assert!(said.contains(" more "), "{said}");
        assert!(said.contains("> wiring"), "{said}");
        assert!(said.contains("  your edits thrown away"), "{said}");
        assert!(said.contains("enter goes on"), "{said}");
    }

    /// An errand that has to be given a name says what it wants, and what has been
    /// typed of it — the same line a question that takes a word gets.
    #[test]
    fn an_errand_taking_a_name_says_what_it_wants_and_what_was_typed() {
        let stage = Stage::Naming {
            errand: sent("restore"),
            asks: "Which backup, by the name it was written under",
            typed: "lemonfiber-full".to_owned(),
        };

        let said = said(&stage, 20, 90);

        assert!(said.contains("Which backup"), "{said}");
        assert!(said.contains("> lemonfiber-full"), "{said}");
        assert!(said.contains("enter asks"), "{said}");
    }

    /// While the core is working out what an errand would do, the box says so
    /// rather than going quiet under a screen that is still gathering.
    #[test]
    fn an_errand_being_weighed_says_what_it_is_waiting_for() {
        let stage = Stage::Weighing {
            errand: sent("reset"),
            typed: String::new(),
        };

        let said = said(&stage, 20, 90);

        assert!(said.contains("your edits thrown away"), "{said}");
        assert!(said.contains("working out what this would do"), "{said}");
    }

    /// What it would do is above the question and never below it. An effect
    /// somebody reads after agreeing is not one they agreed to.
    #[test]
    fn what_an_errand_would_do_is_said_above_the_question_and_not_under_it() {
        let stage = Stage::Agreeing {
            errand: sent("reset"),
            typed: String::new(),
            would: Some(nine()),
        };

        let said = said(&stage, 20, 90);

        let before = said
            .split("Throw away every edit above?")
            .next()
            .unwrap_or_default();
        assert!(before.contains("line 0"), "{said}");
        assert!(said.contains("put lemonfiber's own state back"), "{said}");
        assert!(said.contains("y goes ahead"), "{said}");
        assert!(said.contains("up and down move"), "{said}");
    }

    /// An errand with nothing to say first is one question and no report, and the
    /// hint under it does not offer a movement there is nothing to move through.
    #[test]
    fn an_errand_with_nothing_to_show_first_is_the_question_alone() {
        let stage = Stage::Agreeing {
            errand: sent("seed"),
            typed: String::new(),
            would: None,
        };

        let said = said(&stage, 20, 90);

        assert!(said.contains("Wire the services to each other?"), "{said}");
        assert!(said.contains("y goes ahead"), "{said}");
        assert!(!said.contains("up and down move"), "{said}");
    }

    /// The name an errand was given completes the question, so what is about to be
    /// overwritten is named in the sentence agreeing to it.
    #[test]
    fn the_name_an_errand_was_given_completes_its_question() {
        let stage = Stage::Agreeing {
            errand: sent("restore"),
            typed: "lemonfiber-full-1.tar.gz".to_owned(),
            would: None,
        };

        let said = said(&stage, 20, 90);

        assert!(
            said.contains("Restore from lemonfiber-full-1.tar.gz?"),
            "{said}"
        );
    }

    /// A long report keeps the question on the screen: the box holds back the rows
    /// the question needs rather than filling them, so what is being agreed to is
    /// never the thing scrolled off.
    #[test]
    fn a_long_report_never_pushes_the_question_off_the_box() {
        let stage = Stage::Agreeing {
            errand: sent("reset"),
            typed: String::new(),
            would: Some(nine()),
        };

        let said = said(&stage, 6, 90);

        assert!(said.contains("Throw away every edit above?"), "{said}");
        assert!(said.contains("more lines below"), "{said}");
    }

    /// An errand under way leaves the screen behind it visible and says what is
    /// running on the one line the footer has.
    #[test]
    fn an_errand_under_way_says_so_on_the_footer_and_covers_nothing() {
        let stage = Stage::Doing {
            errand: sent("backup"),
            typed: String::new(),
        };

        assert!(pane(&stage, 20, 80).is_none());
        let footing = text(&footer(&stage, 200));
        assert!(footing.contains("a backup"), "{footing}");
        assert!(footing.contains("still running"), "{footing}");
    }

    /// Leaving mid-errand says what is being waited on, in the same sentence an
    /// action gets — there is one process here and it is the one holding the claim.
    #[test]
    fn leaving_mid_errand_says_what_is_being_waited_on() {
        let stage = Stage::Doing {
            errand: sent("backup"),
            typed: String::new(),
        };

        let said = staying_for(&stage).unwrap_or_default();

        assert!(said.contains("waiting for a backup to finish"), "{said}");
        assert!(said.contains("leave the stack claimed"), "{said}");
    }

    /// A read outstanding claims nothing, so a screen left with one waits for
    /// nothing — including the run that only says what an errand would do.
    #[test]
    fn nothing_is_waited_for_where_an_errand_has_only_been_weighed() {
        let stage = Stage::Weighing {
            errand: sent("reset"),
            typed: String::new(),
        };

        assert!(staying_for(&stage).is_none());
    }
}
