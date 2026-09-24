//! What the dashboard can be asked, and what each question comes to.
//!
//! Every read behind one key, rather than a key each. The screen already answers
//! `q`, `r`, `?` and five actions, and a key per request does not survive being
//! done twice — so what a person opens is the list of what this stack can be
//! asked, and the list is where the next read goes without costing anybody a
//! letter to remember.
//!
//! There are more questions than reads, because some reads are two questions. Most
//! of those pairs are a listing and a narrowing of it — every setting and one
//! setting — and a trace is narrowed twice over, being asked for a show and then for
//! one season of that show. One pair is not a narrowing at all: moving something
//! forward is asked once per object, and neither the stack nor this program is the
//! smaller case of the other. Each pair is two entries here and one request in the
//! parity table either way.
//!
//! No count is written down. Both numbers move whenever a read or a question is
//! added, they are read off this file by nothing, and a sentence that has to be
//! kept in step with a list below it is a sentence that quietly stops being true.
//!
//! A question is given what it needs a word at a time, on a line for each. One read
//! takes two words and the rest take one or none, and the number of lines is read off
//! what the question says it needs rather than kept beside it — so the line an
//! operator is looking at is always the argument the word will fill.
//!
//! Each question is held by the name of the read every surface answers it at. That
//! name is what [`lemonfiber_api::read::table`] turns into one of the core's own
//! commands, and this reaches that table rather than carrying a second one — so a
//! question asked on this screen reaches the command a browser reaches, and a
//! terminal read that could ask something no other surface can ask is not a state
//! this can hold.
//!
//! What a question has to be given before it can be asked is asked of that table
//! too: an empty answer goes to the same translation, and the refusal put in front
//! of the operator is the sentence a browser is answered with.
//!
//! Everything a question does between the key and the answer is here too — the list,
//! the line a word is typed on, and the wait — for the reason each of the other three
//! flows keeps its own: [`super`] routes a press to the flow it belongs to, and what
//! that flow decides belongs beside the list it decides over.
//!
//! What a question *is* is next door in [`shape`], and that is the one part of this
//! the reasoning above does not hold for: the vocabulary is not something the flow
//! decides over, so it was the half that could leave.

mod shape;

pub(crate) use shape::{Narrows, Needed, Question, Wants};

use lemonfiber_api::read::table::{
    named, ALERTS, BANDWIDTH, CATALOGUE, CHECKS, CLIENTS, CONFIG, CREDENTIALS, FORMS, FRONT_DOOR,
    HELD, HISTORY, HOSTING, MIGRATION, OUTBOUND, PROVENANCE, QUALITY, REQUESTS, STORED, STUCK,
    TRACE, UNINSTALL, UPDATE, VERSION,
};
use lemonfiber_core::app::Command;

use super::chooser::Chooser;
use super::disturbing::{self, Widening};
use super::reading::{moved, Reading};
use super::{Press, Stage, Wanted};

/// The key that opens the list of questions.
pub(crate) const KEY: char = 'a';

/// The word the footer puts beside that key.
pub(crate) const HINT: &str = "ask";

/// The question the list opens on.
///
/// Held apart from the rest for the reason the selected choice is: a list built
/// from a slice that might have been empty carries a case for there being no
/// questions, which is not a state this screen can be in.
static OPENS_ON: Question = Question {
    name: "versions",
    about: "this program, the stack it operates, and the container engine",
    read: VERSION,
    needs: Needed::Nothing,
};

/// The questions after it, read from what the stack is towards what it is doing
/// for the people who asked.
///
/// Each narrowing sits under the listing it narrows. The pair is two entries rather
/// than one that takes an optional word, because for five of these reads naming
/// nothing is already a request in its own right — every setting, the whole
/// household, every form, the whole diagnosis — so a line where nothing typed meant
/// the listing would leave no way to refuse an empty one, and an empty one is exactly
/// what somebody who meant to name something has given.
///
/// A trace is the one narrowing whose listing is not a listing. Following a show
/// already answers season by season, so what the narrowing sits under is a report
/// rather than a list — and the season is typed for that very reason: the list it
/// would be picked off is the answer somebody narrowing is trying not to ask for.
static AFTER: &[Question] = &[
    Question {
        name: "how this stack is doing",
        about: "every check that disturbs nothing, and what each one found",
        read: CHECKS,
        needs: Needed::Nothing,
    },
    Question {
        name: "one family of checks",
        about: "narrow that to one family, or to one check by the name its finding gives it",
        read: CHECKS,
        needs: Needed::Typed(&[Wants {
            asks: "Which checks, by the family or by the name a finding gives one",
            narrows: Narrows::Family,
        }]),
    },
    Question {
        name: "forms",
        about: "the forms this stack declares, and what each one is for",
        read: FORMS,
        needs: Needed::Nothing,
    },
    Question {
        name: "what starting one would come to",
        about: "take one of the forms and say what starting it would do",
        read: FORMS,
        needs: Needed::Picked {
            at: FORMS,
            narrows: Narrows::Form,
        },
    },
    Question {
        name: "settings",
        about: "every setting, with credentials withheld",
        read: CONFIG,
        needs: Needed::Nothing,
    },
    Question {
        name: "one setting",
        about: "read one setting by name, withheld where the listing withholds it",
        read: CONFIG,
        needs: Needed::Typed(&[Wants {
            asks: "Which setting, by the name it goes by",
            narrows: Narrows::Setting,
        }]),
    },
    Question {
        name: "quality",
        about: "the quality in force, what it means, and what it costs",
        read: QUALITY,
        needs: Needed::Nothing,
    },
    Question {
        name: "what was asked for",
        about: "what the household asked for, and where each request stands",
        read: REQUESTS,
        needs: Needed::Nothing,
    },
    Question {
        name: "what one person asked for",
        about: "narrow that to one member of the household",
        read: REQUESTS,
        needs: Needed::Typed(&[Wants {
            asks: "Which member, as you would say their name",
            narrows: Narrows::Member,
        }]),
    },
    Question {
        name: "what one person can watch",
        about: "what is already on the shelf for one member, as their own account sees it",
        read: HELD,
        needs: Needed::Typed(&[Wants {
            asks: "Whose shelf, as you would say their name",
            narrows: Narrows::Member,
        }]),
    },
    Question {
        name: "what keeps running without you",
        about: "which long commands this machine keeps going, and whether it says it is",
        read: HOSTING,
        needs: Needed::Nothing,
    },
    Question {
        name: "where the household begins",
        about: "the one address to send somebody who lives here, and why nothing else is",
        read: FRONT_DOOR,
        needs: Needed::Nothing,
    },
    Question {
        name: "what is kept on this machine",
        about: "every file lemonfiber writes, where it is, and why it is kept",
        read: STORED,
        needs: Needed::Nothing,
    },
    Question {
        name: "what you are told about",
        about: "the preset in force, what it means, and any event set apart from it",
        read: ALERTS,
        needs: Needed::Nothing,
    },
    Question {
        name: "what is already on this machine",
        about:
            "the stacks already standing here, the ports they hold, and what could not be taken \
                over",
        read: MIGRATION,
        needs: Needed::Nothing,
    },
    Question {
        name: "what has already been changed",
        about: "every change lemonfiber made, what it did, and how far each could be put back",
        read: HISTORY,
        needs: Needed::Nothing,
    },
    Question {
        name: "which credentials are held",
        about: "every secret in the stack, what uses it, and where it stands — never its value",
        read: CREDENTIALS,
        needs: Needed::Nothing,
    },
    Question {
        name: "how the line is shared",
        about:
            "what the line carries, what the stack takes of it, and whether the clients keep to it",
        read: BANDWIDTH,
        needs: Needed::Nothing,
    },
    Question {
        name: "what to watch on",
        about: "which app to use on each kind of device, and where to use something else",
        read: CLIENTS,
        needs: Needed::Nothing,
    },
    Question {
        name: "where this copy of lemonfiber stands",
        about: "whether anything newer has been released, and the exact command for whatever \
                put this copy here",
        read: UPDATE,
        needs: Needed::Fixed(Narrows::Object, "self"),
    },
    Question {
        name: "what the stack would move to",
        about: "which services have a newer version pinned, how large each step is, and which \
                of them cannot be walked back",
        read: UPDATE,
        needs: Needed::Fixed(Narrows::Object, "stack"),
    },
    Question {
        name: "what leaves this machine",
        about: "every request lemonfiber makes, what it sends, and how to stop each one",
        read: OUTBOUND,
        needs: Needed::Nothing,
    },
    Question {
        name: "what each service is for",
        about: "what every service does for you in plain language, what you lose while it \
                is down, and anything this stack has dropped",
        read: CATALOGUE,
        needs: Needed::Nothing,
    },
    Question {
        name: "where each service comes from",
        about: "the licence every service is published under, the project behind it, and the \
                exact image this stack pins",
        read: PROVENANCE,
        needs: Needed::Nothing,
    },
    Question {
        name: "what is stuck",
        about: "the downloads that have stopped, each one followable",
        read: STUCK,
        needs: Needed::Picked {
            at: TRACE,
            narrows: Narrows::Term,
        },
    },
    Question {
        name: "where one thing is",
        about: "follow one show or film across the services",
        read: TRACE,
        needs: Needed::Typed(&[Wants {
            asks: "What to follow",
            narrows: Narrows::Term,
        }]),
    },
    Question {
        name: "where one season of it is",
        about: "narrow that to one season, instead of every season of the show",
        read: TRACE,
        needs: Needed::Typed(&[
            Wants {
                asks: "What to follow",
                narrows: Narrows::Term,
            },
            Wants {
                asks: "Which season, as a number",
                narrows: Narrows::Season,
            },
        ]),
    },
    // Last, and deliberately: it is the one question here whose answer an operator
    // reads before deciding to stop using this at all, and a list is read from what
    // the stack is towards what becomes of it.
    Question {
        name: "what removing lemonfiber would take",
        about: "every container, image and path one of the four removals would take, and \
                what each occupies",
        read: UNINSTALL,
        needs: Needed::Typed(&[Wants {
            asks: "which removal — stop, services, configuration or media",
            narrows: Narrows::Removal,
        }]),
    },
];

/// What asking a read for one named thing comes to.
///
/// The entry taken off a listing goes through the same table the question itself
/// goes through, so following one stuck item reaches the command a browser reaches
/// asking `/api/trace` for the same title — and an entry carrying nothing to ask by
/// comes to a refusal here rather than to a read of everything.
pub(super) fn asked_at(
    at: &'static str,
    narrows: Narrows,
    names: &str,
) -> Result<Command, &'static str> {
    named(at, narrows.given(names))
}

/// The questions, the one the list opens on apart from the rest.
pub(crate) fn all() -> (&'static Question, Vec<&'static Question>) {
    (&OPENS_ON, AFTER.iter().collect())
}

/// Every question, in the order they are read.
#[cfg(test)]
fn every() -> impl Iterator<Item = &'static Question> {
    std::iter::once(&OPENS_ON).chain(AFTER)
}

/// Over the questions: move, take one, or leave it.
pub(super) fn wondering(
    stage: &mut Stage,
    mut chooser: Chooser<&'static Question>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return take(stage, chooser.taken()),
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Wondering(chooser);
    Wanted::Nothing
}

/// Ask the question that was taken, or open the first line it has to be given.
fn take(stage: &mut Stage, question: &'static Question) -> Wanted {
    asking(stage, question, Vec::new(), String::new())
}

/// Open the next line the question is short of, or put it where it is short of none.
fn asking(
    stage: &mut Stage,
    question: &'static Question,
    said: Vec<String>,
    typed: String,
) -> Wanted {
    if question.needs.asks(said.len()).is_none() {
        return put(stage, question, said);
    }
    *stage = Stage::Typing {
        question,
        said,
        typed,
    };
    Wanted::Nothing
}

/// Over the line being typed: type, take back, go on, or leave it.
///
/// Taking back at an empty line takes back the word before it, which is the only way
/// a question asked two words has of correcting the first — and the same key does it,
/// so nobody has to learn a second one.
pub(super) fn typing(
    stage: &mut Stage,
    question: &'static Question,
    mut said: Vec<String>,
    mut typed: String,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => {
            said.push(typed);
            return asking(stage, question, said, String::new());
        }
        Press::Rubout => back(&mut said, &mut typed),
        Press::Typed(character) => typed.push(character),
        Press::Back | Press::Forward => (),
    }
    *stage = Stage::Typing {
        question,
        said,
        typed,
    };
    Wanted::Nothing
}

/// Take back the last character, or the last word where there is no character left.
fn back(said: &mut Vec<String>, typed: &mut String) {
    if typed.pop().is_none() {
        if let Some(before) = said.pop() {
            *typed = before;
        }
    }
}

/// Put the question to the core, or say why it cannot be put.
///
/// Carried rather than awaited, because a question about what the household asked
/// for reaches the services over the network and a screen that waited on it would
/// stop answering keys while it did.
///
/// A question narrowed by picking is carried the same way and for the same reason —
/// the list of what is stuck is read off the \*arrs — so what comes back is a
/// listing to choose from rather than an answer to read. Which of the two it is is
/// decided where the answer arrives, in [`super::narrowing`].
fn put(stage: &mut Stage, question: &'static Question, said: Vec<String>) -> Wanted {
    match question.command(&said) {
        Ok(command) => {
            *stage = Stage::Waiting { question, said };
            Wanted::Carry(command)
        }
        // A question that reached no command has disturbed nothing and reported
        // nothing, so there is no reading for a widening to be offered under.
        Err(said) => {
            *stage = Stage::Answered {
                question,
                widening: None,
                reading: Reading::of(vec![said.to_owned()]),
            };
            Wanted::Nothing
        }
    }
}

/// Over an answer: move through it, take up what is offered under it, or put it
/// away.
///
/// A reading moves, and any key that is not a move puts it away — the way the pane
/// of words is put away. The one answer with something offered under it is a
/// diagnosis, and there the key that is not a move is the one that widens it, which
/// [`super::disturbing`] decides rather than this.
pub(super) fn answered(
    stage: &mut Stage,
    question: &'static Question,
    widening: Option<Widening>,
    mut reading: Reading,
    press: &Press,
) -> Wanted {
    if moved(&mut reading, press) {
        *stage = Stage::Answered {
            question,
            widening,
            reading,
        };
        return Wanted::Nothing;
    }
    disturbing::answered(stage, widening, press)
}

/// While a question, or one of the things it listed, is with the core: back out, or
/// wait for it.
///
/// Given the stage to go back to rather than building one, because two stages wait
/// the same way and on the same keys — the question itself, and the one of its
/// entries that was taken.
pub(super) fn waiting(stage: &mut Stage, waiting_on: Stage, press: &Press) -> Wanted {
    if matches!(*press, Press::Abandon) {
        return Wanted::Nothing;
    }
    *stage = waiting_on;
    Wanted::Nothing
}

#[cfg(test)]
pub(crate) mod tests;
