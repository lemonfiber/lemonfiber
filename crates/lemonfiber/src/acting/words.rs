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

use super::chooser::Chooser;
use super::offer::{Choice, Offer, OFFERED};
use super::Stage;
use crate::pane::quiet;

/// The keys the screen answers whatever else is open.
const ALWAYS: &str = "q quit   r refresh   ? words";

/// What the box holding a report calls itself.
const CAME: &str = " what it came to ";

/// What the box says while the stack is being asked what there is to act on.
const ASKING: &str = "asking this stack what there is to act on";

/// How to move over the list, and how to leave it.
const CHOOSING: &str = "up and down choose   enter goes on   esc leaves it";

/// How the question before an action is answered.
///
/// Only an explicit yes goes ahead, which is how the teardown's own question is
/// read too: an answer that is neither should land on the thing that changes
/// nothing, and on a screen where one key reaches an action it is a keypress that
/// was not meant that this is guarding against.
const AGREEING: &str = "y goes ahead   any other key changes nothing";

/// How a report is put away.
const CLOSING: &str = "any key closes this";

/// What is said beside a running action, and what leaving does about it.
const WAITING: &str = "still running   q stops watching, and the work goes on";

/// A box over the screen: what it is called, and what it says.
pub(crate) struct Pane {
    /// What the box is called, on its own border.
    pub(crate) title: String,
    /// What it says.
    pub(crate) lines: Vec<Line<'static>>,
}

/// The box over the screen, or nothing where an action has none open.
///
/// A running action has none. The screen behind it is the report — the panels go
/// on gathering every second while the work runs, so what the action is doing to
/// the services shows where the services are listed, and a box over that would
/// take away the one thing worth watching.
pub(super) fn pane(stage: &Stage, rows: usize, across: usize) -> Option<Pane> {
    match stage {
        Stage::Idle | Stage::Running { .. } => None,
        Stage::Asking(offer) => Some(Pane {
            title: titled(offer),
            lines: vec![dimmed(ASKING, across)],
        }),
        Stage::Choosing { offer, chooser } => Some(Pane {
            title: titled(offer),
            lines: choosing(chooser, rows, across),
        }),
        Stage::Confirming { offer, chosen } => Some(Pane {
            title: titled(offer),
            lines: confirming(offer, chosen, across),
        }),
        Stage::Came(said) => Some(Pane {
            title: CAME.to_owned(),
            lines: came(said, rows, across),
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
        _ => keys(),
    };
    dimmed(&said, across)
}

/// Every key this screen answers, in the order they are worth reading.
///
/// Built from the offers rather than written out, so an action added to the table
/// is an action the operator is told about.
fn keys() -> String {
    let mut said = vec![ALWAYS.to_owned()];
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

/// The choices, the selected one marked, and how to move over them.
fn choosing(chooser: &Chooser, rows: usize, across: usize) -> Vec<Line<'static>> {
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

/// One choice: its name, and what it is for beside it.
fn offered(here: bool, choice: &Choice, across: usize) -> Line<'static> {
    let mark = if here { "> " } else { "  " };
    // The marker and the two spaces after the name are taken off before the name is
    // fitted, so the row it ends up on is the width it was given rather than that
    // width plus whatever the marker cost.
    let named = format!(
        "{mark}{}  ",
        shortened(&choice.name, across.saturating_sub(4))
    );
    let room = across.saturating_sub(named.chars().count());
    Line::from(vec![
        Span::raw(named),
        Span::styled(shortened(&choice.about, room), quiet()),
    ])
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

/// What an action came to, in the words the command line gives for the same run.
fn came(said: &[String], rows: usize, across: usize) -> Vec<Line<'static>> {
    let room = rows.saturating_sub(2);
    let mut lines: Vec<Line<'static>> = said
        .iter()
        .take(room)
        .map(|line| Line::raw(shortened(line, across)))
        .collect();
    let left = said.len().saturating_sub(lines.len());
    if left > 0 {
        lines.push(dimmed(
            &format!("{left} more line{} than this screen has room for", s(left)),
            across,
        ));
    }
    lines.push(Line::raw(""));
    lines.push(dimmed(CLOSING, across));
    lines
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
    use super::{came, footer, keys, pane, Offer, Stage};
    use crate::acting::chooser::Chooser;
    use crate::acting::offer::{Choice, OFFERED};
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

    /// A chooser over two, for the list tests.
    fn two() -> Chooser {
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
    /// which is the one thing an operator watching a teardown needs to know.
    #[test]
    fn a_running_action_says_what_leaving_the_screen_does_about_it() {
        let stage = Stage::Running {
            offer: &A_START,
            chosen: a_choice("Full stack", "everything"),
        };

        let said = text(&footer(&stage, 120));

        assert!(said.contains("Full stack"), "{said}");
        assert!(said.contains("still running"), "{said}");
        assert!(said.contains("the work goes on"), "{said}");
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

    #[test]
    fn asking_the_stack_says_what_it_is_waiting_for() {
        let said = said(&Stage::Asking(&A_START), 20, 80);

        assert!(
            said.contains("start"),
            "the box is named for the action: {said}"
        );
        assert!(said.contains("asking this stack"), "{said}");
    }

    /// A report says how to put it away, and counts what it had no room for.
    #[test]
    fn a_report_too_long_for_the_screen_counts_what_it_left_out() {
        let report: Vec<String> = (0..9).map(|at| format!("line {at}")).collect();

        let short: Vec<String> = came(&report, 4, 80).iter().map(text).collect();
        let whole: Vec<String> = came(&report, 40, 80).iter().map(text).collect();

        assert!(
            short.iter().any(|line| line.contains("7 more lines")),
            "{short:?}"
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
            whole.iter().any(|line| line.contains("any key closes")),
            "{whole:?}"
        );
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

    /// A narrow screen shortens rather than running past its edge.
    #[test]
    fn no_row_runs_past_the_edge_of_a_narrow_screen() {
        let stage = Stage::Choosing {
            offer: &A_START,
            chooser: two(),
        };

        for across in [4usize, 12, 24, 40] {
            for row in said(&stage, 20, across).lines().skip(1) {
                assert!(row.chars().count() <= across, "at {across}: {row:?}");
            }
        }
    }
}
