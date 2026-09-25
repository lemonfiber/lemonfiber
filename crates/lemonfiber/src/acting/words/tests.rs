use super::{footer, pane, staying_for, surface, Asked, Offer, Open, Stage};
use crate::acting::chooser::Chooser;
use crate::acting::disturbing::{under, Widening, DIAGNOSIS};
use crate::acting::errand::{self, Errand, Given};
use crate::acting::mending::{self, Mending};
use crate::acting::narrowing::Subject;
use crate::acting::offer::{Choice, Taken};
use crate::acting::question::{Narrows, Needed, Question, Wants};
use crate::acting::reading::Reading;
use crate::acting::Press;
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
    needs: Needed::Typed(&[Wants {
        asks: "What to follow",
        narrows: Narrows::Term,
    }]),
};

/// One question that is narrowed by taking an entry off its own listing.
static A_STUCK: Question = Question {
    name: "what is stuck",
    about: "the downloads that have stopped, each one followable",
    read: "/api/stuck",
    needs: Needed::Picked {
        at: "/api/trace",
        narrows: Narrows::Term,
    },
};

/// The question that answers with a diagnosis, which is the one answer this
/// screen offers anything under.
static A_DIAGNOSIS: Question = Question {
    name: "how this stack is doing",
    about: "every check that disturbs nothing, and what each one found",
    read: "/api/checks",
    needs: Needed::Nothing,
};

/// The widened run offered under that answer.
fn a_widening() -> Option<Widening> {
    under(&A_DIAGNOSIS, &[])
}

/// A reading over nine numbered lines.
fn nine() -> Reading {
    Reading::of((0..9).map(|at| format!("line {at}")).collect())
}

/// A choice by name, for the tests that only read what is drawn.
fn a_choice(name: &str, about: &str) -> Choice {
    Choice {
        name: name.to_owned(),
        about: about.to_owned(),
        names: vec![name.to_owned()],
        marked: Some(false),
        command: Command::Up {
            forms: vec![name.to_owned()],
        },
    }
}

/// One choice taken on its own, which is what a list with nothing marked on it
/// comes to when enter is pressed over that row.
fn a_taking(name: &str, about: &str) -> Taken {
    Taken {
        command: Command::Up {
            forms: vec![name.to_owned()],
        },
        covers: vec![a_choice(name, about)],
        each: "form",
    }
}

/// Several taken together, which is what marking them and pressing enter comes
/// to.
fn several() -> Taken {
    Taken {
        command: Command::Up {
            forms: vec!["full".to_owned(), "lean".to_owned()],
        },
        covers: vec![
            a_choice("Full stack", "everything, behind the tunnel"),
            a_choice("Lean stack", "the download clients only"),
        ],
        each: "form",
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
fn two_listed() -> Chooser<Subject> {
    Chooser::over(
        a_subject("Full stack", "everything, behind the tunnel"),
        vec![a_subject("Lean stack", "the download clients only")],
    )
}

/// One of the things a listing offered, the command behind it beside the point
/// for a test that only reads what is drawn.
fn a_subject(name: &str, about: &str) -> Subject {
    Subject {
        name: name.to_owned(),
        about: about.to_owned(),
        command: Command::Stuck,
    }
}

fn two() -> Chooser<Choice> {
    Chooser::over(
        a_choice("Full stack", "everything, behind the tunnel"),
        vec![a_choice("Lean stack", "the download clients only")],
    )
}

mod errands;
mod questions;
mod repairs;
