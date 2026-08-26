//! What the dashboard can be asked, and what each question comes to.
//!
//! Nine reads behind one key, rather than nine keys. The screen already answers
//! `q`, `r`, `?` and five actions, and a key per request does not survive being
//! done twice — so what a person opens is the list of what this stack can be
//! asked, and the list is where a tenth read would go without costing anybody a
//! letter to remember. Fourteen questions sit over those nine, because four of the
//! reads are asked both whole and narrowed and each of those pairs is two entries
//! on the list and one request in the parity table either way — and one of them is
//! narrowed twice over, a trace being asked for a show and then for one season of
//! that show.
//!
//! A question is given what it needs a word at a time, on a line for each. One read
//! takes two words and the rest take one or none, and the number of lines is read off
//! what the question says it needs rather than kept beside it — so the line an
//! operator is looking at is always the argument the word will fill.
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
    named, Wanted as Asking, CHECKS, CONFIG, FORMS, FRONT_DOOR, OUTBOUND, QUALITY, REQUESTS, STUCK,
    TRACE, VERSION,
};
use lemonfiber_core::app::Command;

use super::chooser::{Chooser, Listed};
use super::disturbing::{self, Widening};
use super::reading::{moved, Reading};
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
    /// The family of checks, or the one check, to narrow a diagnosis to.
    Family,
    /// The season of a followed show to narrow to, instead of every season.
    ///
    /// Carried as it was written rather than as a number. The read parses it, and a
    /// season that is not a number is refused there — so a line typed at this screen
    /// and a query string carrying the same word are answered in one sentence.
    Season,
}

impl Narrows {
    /// The read's arguments as they now stand, with this one filled by what was
    /// given.
    ///
    /// Filled into what is already there rather than built fresh, because a question
    /// may be given more than one word and each fills an argument of its own. A
    /// narrowing that returned a whole set of arguments could only be the last one
    /// asked for.
    pub(super) fn fill(self, said: &str, into: &mut Asking) {
        let said = said.to_owned();
        match self {
            Self::Term => into.term = Some(said),
            Self::Setting => into.key = Some(said),
            Self::Member => into.member = Some(said),
            Self::Form => into.forms = vec![said],
            Self::Family => into.only = Some(said),
            Self::Season => into.season = Some(said),
        }
    }

    /// The read's own arguments, with this one filled and nothing else.
    pub(super) fn given(self, said: &str) -> Asking {
        let mut given = Asking::default();
        self.fill(said, &mut given);
        given
    }
}

/// One word a question has to be given, and what that word is for.
pub(crate) struct Wants {
    /// What is asked for, above the line it is typed on.
    pub(crate) asks: &'static str,
    /// Which of the read's arguments it fills.
    pub(crate) narrows: Narrows,
}

/// What a question has to be given before it can be asked.
///
/// Three shapes, and which one a question takes is a question about where the thing
/// being named already is. A title nobody has listed is typed. A setting, a member
/// and a season are typed too, because the list each would be picked off is the very
/// answer the narrowing exists to avoid asking for. A form or a stuck item is taken
/// off a list the screen can have in hand for the asking, and picking one is exact
/// where typing it would be a spelling test.
pub(crate) enum Needed {
    /// Nothing: it is asked as it stands.
    Nothing,
    /// A word for each of these, each on a line of its own, filling one of the
    /// read's arguments.
    ///
    /// A slice rather than a single word because one read takes two: following a
    /// show is a title, and following one season of it is that title and a number.
    /// Two entries on this list rather than a second flow, so the line a title is
    /// typed on is the line a title is typed on wherever it is asked for.
    Typed(&'static [Wants]),
    /// One of the entries this question's own read comes back with.
    ///
    /// The entry taken is asked at [`Needed::Picked::at`], which need not be the read
    /// that listed it: what is stuck is listed by one read and each entry followed by
    /// another, exactly as the web's own list of stuck items links to its traces.
    Picked {
        /// The read the entry taken is asked at.
        at: &'static str,
        /// Which of that read's arguments the entry fills.
        narrows: Narrows,
    },
}

impl Needed {
    /// What is asked for on the line after the words already said, or nothing where
    /// the question has everything it needs.
    ///
    /// Answered by how many words are in hand rather than by a field on the stage, so
    /// the line an operator is looking at and the argument what they type will fill
    /// cannot come apart.
    pub(crate) fn asks(&self, said: usize) -> Option<&'static str> {
        match self {
            Self::Nothing | Self::Picked { .. } => None,
            Self::Typed(asking) => asking.get(said).map(|wants| wants.asks),
        }
    }

    /// Where an entry taken off a listing is asked, and which argument it fills.
    pub(super) const fn picking(&self) -> Option<(&'static str, Narrows)> {
        match self {
            Self::Nothing | Self::Typed(_) => None,
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
    pub(crate) fn command(&self, said: &[String]) -> Result<Command, &'static str> {
        let mut given = Asking::default();
        if let Needed::Typed(asking) = &self.needs {
            for (wants, word) in asking.iter().zip(said) {
                wants.narrows.fill(word, &mut given);
            }
        }
        named(self.read, given)
    }

    /// What the line being typed at is asking for.
    ///
    /// A question is only ever being typed at while it is short of a word, so what it
    /// is short of is what the line asks for. One that is short of none has already
    /// been put to the core and is no longer a line anybody is looking at.
    pub(crate) fn asking(&self, said: usize) -> &'static str {
        self.needs.asks(said).unwrap_or_default()
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
        name: "where the household begins",
        about: "the one address to send somebody who lives here, and why nothing else is",
        read: FRONT_DOOR,
        needs: Needed::Nothing,
    },
    Question {
        name: "what leaves this machine",
        about: "every request lemonfiber makes, what it sends, and how to stop each one",
        read: OUTBOUND,
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
pub(crate) mod tests {
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

    /// A question's command, given the words typed at its lines in order.
    ///
    /// Through the question rather than through the translation, because what is
    /// being asserted is the pair: which arguments this screen fills, and what the
    /// table makes of them.
    fn asking(question: &'static Question, said: &[&str]) -> Result<Command, &'static str> {
        let said: Vec<String> = said.iter().map(|word| (*word).to_owned()).collect();
        question.command(&said)
    }

    /// The question by that name, which is how each one below is reached.
    pub(crate) fn called(name: &str) -> &'static Question {
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
            .map(|question| asking(question, &["SONARR_API_KEY"]))
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
            asking(called("where one thing is"), &["The Expanse"]),
            Ok(Command::Trace {
                term: "The Expanse".to_owned(),
                season: None,
            })
        );
        assert_eq!(
            asking(called("one setting"), &["The Expanse"]),
            Ok(Command::ConfigGet {
                key: "The Expanse".to_owned(),
            })
        );
        assert_eq!(
            asking(called("what one person asked for"), &["The Expanse"]),
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
            asking(called("where one thing is"), &[""]),
            Err(NO_TERM),
            "a trace with nothing typed"
        );
        assert_eq!(
            asking(called("one setting"), &[""]),
            Err(NO_SETTING),
            "a setting with nothing typed"
        );
        assert_eq!(
            asking(called("what one person asked for"), &[""]),
            Err(NO_MEMBER),
            "a member with nothing typed"
        );
    }

    /// Naming nothing is a request in its own right for the reads that have a
    /// listing, which is why the narrowing is a second question rather than an empty
    /// line on the first one.
    #[test]
    fn the_listing_beside_each_narrowing_still_asks_for_everything() {
        assert_eq!(asking(called("settings"), &[""]), Ok(Command::ConfigShow));
        assert_eq!(
            asking(called("what was asked for"), &[""]),
            Ok(Command::Household { member: None })
        );
        assert_eq!(asking(called("forms"), &[""]), Ok(Command::Forms));
    }

    /// A question narrowed by picking asks its own read for the whole listing first,
    /// and that listing is the question given nothing rather than a second command
    /// written down beside it.
    #[test]
    fn a_question_that_picks_asks_its_read_for_the_listing_first() {
        assert_eq!(
            asking(called("what starting one would come to"), &[""]),
            Ok(Command::Forms)
        );
        assert_eq!(asking(called("what is stuck"), &[""]), Ok(Command::Stuck));
        // Typing at one of these is not how it is narrowed, so a word reaching here
        // changes nothing about what is asked.
        assert_eq!(
            asking(called("what is stuck"), &["ignored"]),
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

    /// What is asked for above the line is asked for only where there is a line, and
    /// the line asked for is the one the words in hand have got to.
    ///
    /// A question given everything it needs asks for nothing more, which is how the
    /// flow knows to stop opening lines and put the question instead.
    #[test]
    fn only_a_typed_question_asks_for_something_above_a_line() {
        assert_eq!(
            called("one setting").needs.asks(0),
            Some("Which setting, by the name it goes by")
        );
        assert!(called("one setting").needs.asks(1).is_none());
        assert!(called("what is stuck").needs.asks(0).is_none());
        assert!(called("settings").needs.asks(0).is_none());
        assert!(called("settings").needs.picking().is_none());
        assert!(called("where one thing is").needs.picking().is_none());
    }

    /// A question given two words asks for the second only once the first is in hand,
    /// and the line an operator is looking at names the argument that word will fill.
    ///
    /// The claim rather than a consequence: the prompt is derived from how many words
    /// there are, so a question that asked for the season first would fail here.
    #[test]
    fn a_question_given_two_words_asks_for_them_one_line_at_a_time() {
        let season = called("where one season of it is");

        assert_eq!(season.needs.asks(0), Some("What to follow"));
        assert_eq!(season.needs.asks(1), Some("Which season, as a number"));
        assert!(season.needs.asks(2).is_none());
        assert_eq!(season.asking(1), "Which season, as a number");
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
