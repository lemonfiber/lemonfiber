//! What the dashboard can be asked, and what each question comes to.
//!
//! Six questions behind one key, rather than six keys. The screen already answers
//! `q`, `r`, `?` and five actions, and a key per request does not survive being
//! done twice — so what a person opens is the list of what this stack can be
//! asked, and the list is where a seventh question would go without costing
//! anybody a letter to remember.
//!
//! Each question is held by the name of the read every surface answers it at. That
//! name is what [`lemonfiber_api::reads`] turns into one of the core's own
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

use lemonfiber_api::reads::{
    named, Wanted as Asking, CONFIG, FORMS, QUALITY, REQUESTS, TRACE, VERSION,
};
use lemonfiber_core::app::Command;

use super::chooser::{Chooser, Listed};
use super::reading::Reading;
use super::{Press, Stage, Wanted};

/// The key that opens the list of questions.
pub(crate) const KEY: char = 'a';

/// The word the footer puts beside that key.
pub(crate) const HINT: &str = "ask";

/// What a question has to be given before it can be asked.
pub(crate) enum Needed {
    /// Nothing: it is asked as it stands.
    Nothing,
    /// The words naming what to follow, under the line they are typed on.
    Term(&'static str),
}

impl Needed {
    /// What is asked for above the line being typed, or nothing where a question
    /// takes none.
    pub(crate) const fn asks(&self) -> Option<&'static str> {
        match self {
            Self::Nothing => None,
            Self::Term(asks) => Some(*asks),
        }
    }
}

/// One question the dashboard can put to this stack.
pub(crate) struct Question {
    /// What it is called on the list.
    pub(crate) name: &'static str,
    /// What it answers, in one line.
    pub(crate) about: &'static str,
    /// The read every surface answers it at.
    pub(crate) read: &'static str,
    /// What it has to be given first.
    pub(crate) needs: Needed,
}

impl Question {
    /// The command this question comes to, or why it comes to none.
    ///
    /// The word typed goes to the read's own arguments and the translation decides
    /// what to do with it, so an empty one is refused in the sentence the web
    /// surface gives for the same request rather than in one written here.
    pub(crate) fn command(&self, typed: &str) -> Result<Command, &'static str> {
        let given = match self.needs {
            Needed::Nothing => Asking::default(),
            Needed::Term(_) => Asking {
                term: Some(typed.to_owned()),
                ..Asking::default()
            },
        };
        named(self.read, given)
    }
}

impl Listed for Question {
    fn name(&self) -> &str {
        self.name
    }

    fn about(&self) -> &str {
        self.about
    }
}

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
static AFTER: &[Question] = &[
    Question {
        name: "forms",
        about: "the forms this stack declares, and what each one is for",
        read: FORMS,
        needs: Needed::Nothing,
    },
    Question {
        name: "settings",
        about: "every setting, with credentials withheld",
        read: CONFIG,
        needs: Needed::Nothing,
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
        name: "where one thing is",
        about: "follow one show or film across the services",
        read: TRACE,
        needs: Needed::Term("What to follow"),
    },
];

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

/// Ask the question that was taken, or open the line it has to be given first.
fn take(stage: &mut Stage, question: &'static Question) -> Wanted {
    match question.needs.asks() {
        Some(asks) => {
            *stage = Stage::Typing {
                question,
                asks,
                typed: String::new(),
            };
            Wanted::Nothing
        }
        None => put(stage, question, ""),
    }
}

/// Over the line being typed: type, take back, ask, or leave it.
pub(super) fn typing(
    stage: &mut Stage,
    question: &'static Question,
    asks: &'static str,
    mut typed: String,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return put(stage, question, &typed),
        Press::Rubout => {
            typed.pop();
        }
        Press::Typed(character) => typed.push(character),
        Press::Back | Press::Forward => (),
    }
    *stage = Stage::Typing {
        question,
        asks,
        typed,
    };
    Wanted::Nothing
}

/// Put the question to the core, or say why it cannot be put.
///
/// Carried rather than awaited, because a question about what the household asked
/// for reaches the services over the network and a screen that waited on it would
/// stop answering keys while it did.
fn put(stage: &mut Stage, question: &'static Question, typed: &str) -> Wanted {
    match question.command(typed) {
        Ok(command) => {
            *stage = Stage::Waiting(question);
            Wanted::Carry(command)
        }
        Err(said) => {
            *stage = Stage::Answered {
                question,
                reading: Reading::of(vec![said.to_owned()]),
            };
            Wanted::Nothing
        }
    }
}

/// While the question is with the core: back out, or wait for it.
pub(super) fn waiting(stage: &mut Stage, question: &'static Question, press: &Press) -> Wanted {
    if matches!(*press, Press::Abandon) {
        return Wanted::Nothing;
    }
    *stage = Stage::Waiting(question);
    Wanted::Nothing
}

#[cfg(test)]
mod tests {
    use super::{all, every, Needed, CONFIG};
    use lemonfiber_api::reads::{NO_TERM, OFFERED as SERVED};
    use lemonfiber_core::app::Command;

    /// The whole point of naming the read rather than assembling a command here:
    /// what this screen asks has to be something another surface already answers,
    /// or the requirement it is being built for is defeated by the thing built for
    /// it.
    #[test]
    fn every_question_this_screen_asks_is_one_the_other_surfaces_answer() {
        let missing: Vec<&str> = every()
            .map(|question| question.read)
            .filter(|read| !SERVED.contains(read))
            .collect();

        assert!(missing.is_empty(), "{missing:?}");
    }

    /// A question is named once, or the second is unreachable on a list that shows
    /// both and nobody would know which they took.
    #[test]
    fn no_two_questions_go_by_the_same_name() {
        for question in every() {
            let same = every().filter(|other| other.name == question.name).count();
            assert_eq!(
                same, 1,
                "more than one question is called {}",
                question.name
            );
        }
    }

    /// Every question says what it answers, since the line under the name is the
    /// whole of what somebody chooses between them on.
    #[test]
    fn every_question_says_what_it_answers() {
        for question in every() {
            assert!(!question.about.is_empty(), "{}", question.name);
        }
    }

    /// The settings are read by asking the core for them, which is where the
    /// withholding happens. A screen that reached the file itself would be outside
    /// that path and would show the values it exists to keep out of a report.
    #[test]
    fn the_settings_question_asks_for_the_reading_that_withholds() {
        let asking = every().find(|question| question.read == CONFIG);

        assert_eq!(
            asking.map(|question| question.command("")),
            Some(Ok(Command::ConfigShow))
        );
    }

    /// A question that takes a word comes to a command once it has one, and says
    /// what is missing while it has not — in the words the other surface gives.
    #[test]
    fn a_question_that_takes_a_word_is_refused_until_it_has_one() {
        let question = every().find(|question| question.needs.asks().is_some());

        assert_eq!(
            question.map(|question| question.command("The Expanse")),
            Some(Ok(Command::Trace {
                term: "The Expanse".to_owned(),
                season: None,
            }))
        );
        assert_eq!(
            question.map(|question| question.command("")),
            Some(Err(NO_TERM))
        );
    }

    /// The one thing this list is checked against from outside the binary. A
    /// question offered here with no entry there leaves the parity table's terminal
    /// column claiming less than the screen does, and an entry there naming a read
    /// nothing asks leaves it claiming more.
    #[test]
    fn every_question_this_screen_asks_is_published_for_the_parity_table() {
        let mut asked: Vec<&str> = every().map(|question| question.read).collect();
        let mut published: Vec<&str> = lemonfiber::reaching::ASKS
            .iter()
            .map(|reach| reach.through)
            .collect();
        asked.sort_unstable();
        published.sort_unstable();

        assert_eq!(asked, published);
    }

    /// The list opens on the first question and holds every one of them.
    #[test]
    fn the_list_opens_on_the_first_question_and_holds_them_all() {
        let (first, rest) = all();

        assert_eq!(first.name, "versions");
        assert_eq!(rest.len() + 1, every().count());
        assert!(matches!(first.needs, Needed::Nothing));
    }
}
