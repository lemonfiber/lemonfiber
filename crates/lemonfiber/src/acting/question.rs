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
    named, Wanted as Asking, CONFIG, FORMS, QUALITY, REQUESTS, STUCK, TRACE, VERSION,
};
use lemonfiber_core::app::Command;

use super::chooser::{Chooser, Listed};
use super::reading::Reading;
use super::{Press, Stage, Wanted};

/// The key that opens the list of questions.
pub(crate) const KEY: char = 'a';

/// The word the footer puts beside that key.
pub(crate) const HINT: &str = "ask";

/// Which of a read's own arguments a narrowing fills.
///
/// Named rather than assembled. What is typed or taken at this screen goes into the
/// field [`lemonfiber_api::reads::Wanted`] names for that read, and the translation
/// decides what to do with it — so a narrowing offered here is one a query string
/// can carry, and one it cannot is not a state this can hold. It is also what keeps
/// the refusals honest: an argument left empty is refused in the sentence the same
/// argument left empty in a query string is refused with, rather than in one this
/// screen wrote.
#[derive(Clone, Copy)]
pub(crate) enum Narrows {
    /// What to follow, named as a person would say it.
    Term,
    /// The setting to read, instead of every one of them.
    Setting,
    /// The household member to narrow to.
    Member,
    /// The form to say what starting would come to.
    Form,
}

impl Narrows {
    /// The read's own arguments, with this one filled by what was given.
    pub(super) fn given(self, said: &str) -> Asking {
        let said = said.to_owned();
        match self {
            Self::Term => Asking {
                term: Some(said),
                ..Asking::default()
            },
            Self::Setting => Asking {
                key: Some(said),
                ..Asking::default()
            },
            Self::Member => Asking {
                member: Some(said),
                ..Asking::default()
            },
            Self::Form => Asking {
                forms: vec![said],
                ..Asking::default()
            },
        }
    }
}

/// What a question has to be given before it can be asked.
///
/// Three shapes, and which one a question takes is a question about where the thing
/// being named already is. A title nobody has listed is typed. A setting or a member
/// is typed too, because the list it would be picked off is the very answer the
/// narrowing exists to avoid asking for. A form or a stuck item is taken off a list
/// the screen can have in hand for the asking, and picking one is exact where typing
/// it would be a spelling test.
pub(crate) enum Needed {
    /// Nothing: it is asked as it stands.
    Nothing,
    /// A word, under the line it is typed on, filling one of the read's arguments.
    Typed {
        /// What is asked for, above the line it is typed on.
        asks: &'static str,
        /// Which of the read's arguments it fills.
        narrows: Narrows,
    },
    /// One of the entries this question's own read comes back with.
    ///
    /// The entry taken is asked at [`Picked::at`], which need not be the read that
    /// listed it: what is stuck is listed by one read and each entry followed by
    /// another, exactly as the web's own list of stuck items links to its traces.
    Picked {
        /// The read the entry taken is asked at.
        at: &'static str,
        /// Which of that read's arguments the entry fills.
        narrows: Narrows,
    },
}

impl Needed {
    /// What is asked for above the line being typed, or nothing where a question
    /// takes no typing.
    pub(crate) const fn asks(&self) -> Option<&'static str> {
        match self {
            Self::Nothing | Self::Picked { .. } => None,
            Self::Typed { asks, .. } => Some(*asks),
        }
    }

    /// Where an entry taken off a listing is asked, and which argument it fills.
    pub(super) const fn picking(&self) -> Option<(&'static str, Narrows)> {
        match self {
            Self::Nothing | Self::Typed { .. } => None,
            Self::Picked { at, narrows } => Some((*at, *narrows)),
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
    ///
    /// A question narrowed by picking comes here with nothing typed, and what it
    /// comes to is that read asked for its whole listing — which is the list the
    /// operator is about to take one of. So the listing is not a second command
    /// written down beside this one; it is this one, given nothing.
    pub(crate) fn command(&self, typed: &str) -> Result<Command, &'static str> {
        let given = match self.needs {
            Needed::Nothing | Needed::Picked { .. } => Asking::default(),
            Needed::Typed { narrows, .. } => narrows.given(typed),
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
///
/// Each narrowing sits under the listing it narrows. The pair is two entries rather
/// than one that takes an optional word, because for four of these reads naming
/// nothing is already a request in its own right — every setting, the whole
/// household, every form — so a line where nothing typed meant the listing would
/// leave no way to refuse an empty one, and an empty one is exactly what somebody
/// who meant to name something has given.
static AFTER: &[Question] = &[
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
        needs: Needed::Typed {
            asks: "Which setting, by the name it goes by",
            narrows: Narrows::Setting,
        },
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
        needs: Needed::Typed {
            asks: "Which member, as you would say their name",
            narrows: Narrows::Member,
        },
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
        needs: Needed::Typed {
            asks: "What to follow",
            narrows: Narrows::Term,
        },
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
///
/// A question narrowed by picking is carried the same way and for the same reason —
/// the list of what is stuck is read off the \*arrs — so what comes back is a
/// listing to choose from rather than an answer to read. Which of the two it is is
/// decided where the answer arrives, in [`super::narrowing`].
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
mod tests {
    use super::{all, asked_at, every, Narrows, Needed, Question, CONFIG, FORMS, OPENS_ON, TRACE};
    use lemonfiber_api::reads::{NO_MEMBER, NO_SETTING, NO_TERM, OFFERED as SERVED};
    use lemonfiber_core::app::Command;
    use std::collections::BTreeSet;

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

    /// The question by that name, which is how each one below is reached.
    fn called(name: &str) -> &'static Question {
        every()
            .find(|question| question.name == name)
            .unwrap_or(&OPENS_ON)
    }

    /// The settings are read by asking the core for them, which is where the
    /// withholding happens. A screen that reached the file itself would be outside
    /// that path and would show the values it exists to keep out of a report.
    ///
    /// Both halves of the settings, because naming one is where a way past the
    /// withholding would be built if it were built anywhere: `ConfigGet` narrows what
    /// `config show` displayed rather than displaying what it found, so a named
    /// credential is withheld exactly as the listing withholds it.
    #[test]
    fn both_settings_questions_ask_for_the_reading_that_withholds() {
        // Every question over this read rather than the two by name, so a third
        // added later is not quietly a third way to reach the settings.
        let asking: Vec<Result<Command, &str>> = every()
            .filter(|question| question.read == CONFIG)
            .map(|question| question.command("SONARR_API_KEY"))
            .collect();

        assert_eq!(
            asking,
            vec![
                Ok(Command::ConfigShow),
                Ok(Command::ConfigGet {
                    key: "SONARR_API_KEY".to_owned(),
                }),
            ]
        );
    }

    /// Every question that takes a word fills the argument its own read names, and
    /// each one comes to a different command for the same word typed.
    ///
    /// The whole of what the generalisation bought: one line to type on, and which of
    /// the read's arguments it fills said once, beside the question.
    #[test]
    fn each_typed_question_fills_the_argument_its_read_names() {
        assert_eq!(
            called("where one thing is").command("The Expanse"),
            Ok(Command::Trace {
                term: "The Expanse".to_owned(),
                season: None,
            })
        );
        assert_eq!(
            called("one setting").command("The Expanse"),
            Ok(Command::ConfigGet {
                key: "The Expanse".to_owned(),
            })
        );
        assert_eq!(
            called("what one person asked for").command("The Expanse"),
            Ok(Command::Household {
                member: Some("The Expanse".to_owned()),
            })
        );
    }

    /// Nothing typed is refused, and refused in the sentence the same read refuses a
    /// browser with rather than in one this screen wrote.
    ///
    /// Three different sentences for three different arguments, which is the point:
    /// each comes from the translation that knows what was being named.
    #[test]
    fn a_question_that_takes_a_word_is_refused_until_it_has_one() {
        assert_eq!(
            called("where one thing is").command(""),
            Err(NO_TERM),
            "a trace with nothing typed"
        );
        assert_eq!(
            called("one setting").command(""),
            Err(NO_SETTING),
            "a setting with nothing typed"
        );
        assert_eq!(
            called("what one person asked for").command(""),
            Err(NO_MEMBER),
            "a member with nothing typed"
        );
    }

    /// Naming nothing is a request in its own right for the reads that have a
    /// listing, which is why the narrowing is a second question rather than an empty
    /// line on the first one.
    #[test]
    fn the_listing_beside_each_narrowing_still_asks_for_everything() {
        assert_eq!(called("settings").command(""), Ok(Command::ConfigShow));
        assert_eq!(
            called("what was asked for").command(""),
            Ok(Command::Household { member: None })
        );
        assert_eq!(called("forms").command(""), Ok(Command::Forms));
    }

    /// A question narrowed by picking asks its own read for the whole listing first,
    /// and that listing is the question given nothing rather than a second command
    /// written down beside it.
    #[test]
    fn a_question_that_picks_asks_its_read_for_the_listing_first() {
        assert_eq!(
            called("what starting one would come to").command(""),
            Ok(Command::Forms)
        );
        assert_eq!(called("what is stuck").command(""), Ok(Command::Stuck));
        // Typing at one of these is not how it is narrowed, so a word reaching here
        // changes nothing about what is asked.
        assert_eq!(
            called("what is stuck").command("ignored"),
            Ok(Command::Stuck)
        );
    }

    /// Taking one of the listed entries asks the read that entry is asked at, which
    /// need not be the read that listed it.
    #[test]
    fn taking_a_listed_entry_asks_the_read_it_names() {
        let (at, narrows) = called("what is stuck")
            .needs
            .picking()
            .unwrap_or((TRACE, Narrows::Term));

        assert_eq!(at, TRACE, "a stuck item is followed at the trace");
        assert_eq!(
            asked_at(at, narrows, "The Expanse"),
            Ok(Command::Trace {
                term: "The Expanse".to_owned(),
                season: None,
            })
        );

        let (at, narrows) = called("what starting one would come to")
            .needs
            .picking()
            .unwrap_or((FORMS, Narrows::Form));
        assert_eq!(
            asked_at(at, narrows, "full"),
            Ok(Command::Preview {
                forms: vec!["full".to_owned()],
            })
        );
    }

    /// An entry carrying nothing to ask by comes to a refusal rather than to a read
    /// of everything — which is what keeps it off the list offered to the operator.
    #[test]
    fn a_listed_entry_naming_nothing_is_refused_rather_than_followed() {
        assert_eq!(asked_at(TRACE, Narrows::Term, ""), Err(NO_TERM));
    }

    /// What is asked for above the line is asked for only where there is a line.
    #[test]
    fn only_a_typed_question_asks_for_something_above_a_line() {
        assert_eq!(
            called("one setting").needs.asks(),
            Some("Which setting, by the name it goes by")
        );
        assert!(called("what is stuck").needs.asks().is_none());
        assert!(called("settings").needs.asks().is_none());
        assert!(called("settings").needs.picking().is_none());
        assert!(called("where one thing is").needs.picking().is_none());
    }

    /// The one thing this list is checked against from outside the binary. A
    /// question offered here with no entry there leaves the parity table's terminal
    /// column claiming less than the screen does, and an entry there naming a read
    /// nothing asks leaves it claiming more.
    ///
    /// The reads rather than the questions, and there are fewer of the first than of
    /// the second: four reads are asked both whole and narrowed, which is two entries
    /// on this list and one request in the table either way. So this compares the
    /// reads without their repeats — a set against a list is what would otherwise
    /// fail here for a reason that has nothing to do with parity.
    #[test]
    fn every_question_this_screen_asks_is_published_for_the_parity_table() {
        let asked: BTreeSet<&str> = every().map(|question| question.read).collect();
        let published: BTreeSet<&str> = lemonfiber::reaching::ASKS
            .iter()
            .map(|reach| reach.through)
            .collect();

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
