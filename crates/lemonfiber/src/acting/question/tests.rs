use super::{all, asked_at, every, Narrows, Needed, Question, CONFIG, FORMS, OPENS_ON, TRACE};
use lemonfiber_api::read::table::{
    NO_MEMBER, NO_SETTING, NO_SUCH_REMOVAL, NO_TERM, OFFERED as SERVED,
};
use lemonfiber_core::app::Command;
use lemonfiber_core::uninstall::Tier;
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

/// One read, two questions, and the object each supplies for itself.
///
/// Neither is typed and neither is picked: which of the two things can be moved
/// forward is part of what the question *is*, so the list carries two entries
/// rather than one that would make somebody choose again after choosing.
#[test]
fn each_question_that_names_its_own_object_reaches_the_command_for_that_object() {
    assert_eq!(
        asking(called("where this copy of lemonfiber stands"), &[]),
        Ok(Command::SelfUpdate { to: None }),
        "the binary's half, asked with nothing typed"
    );

    let stack = asking(called("what the stack would move to"), &[]);
    assert!(
        matches!(stack, Ok(Command::Update(ref asked)) if !asked.confirm),
        "the services' half, and a read never confirms: {stack:?}"
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
            searching: false,
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
    // The one word this question takes names one of four rather than anything
    // typed, so what it fills is asserted on the removal it comes to.
    assert!(
        matches!(
            asking(called("what removing lemonfiber would take"), &["media"]),
            Ok(Command::Uninstall(ref asked)) if asked.tier == Tier::Media
        ),
        "the word typed did not reach the removal it names"
    );
}

/// A word that names none of the four is refused by name rather than read as the
/// safest of them — somebody who wrote a word and meant it must not be given a
/// different removal because of a spelling.
#[test]
fn a_removal_this_build_does_not_know_is_refused_rather_than_guessed_at() {
    assert_eq!(
        asking(
            called("what removing lemonfiber would take"),
            &["everything"]
        ),
        Err(NO_SUCH_REMOVAL)
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
            searching: false,
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
