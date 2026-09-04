//! What an invitation lets somebody watch, decided while they are being invited.
//!
//! `Command::Invite` is four answers: who it is for, which libraries they may open,
//! how far up the ratings they may go, and what happens to content the media server
//! has no rating for. This screen carried the first and sent nothing for the rest —
//! which is the right thing to send when nobody has been asked, and is not the same as
//! being the only answer available.
//!
//! They are asked in one run because that is the whole point of asking them here. An
//! account made open and narrowed afterwards is open for as long as it takes anybody
//! to remember, and the person most likely to be given a limit has already been handed
//! the address.
//!
//! **The fourth is asked only where something was narrowed.** An offer that names
//! neither a library nor a limit writes no policy at all, so what would happen to
//! unrated content is a question about a setting this run does not touch — and a
//! keypress the ordinary case does not owe.
//!
//! **The libraries are typed and the age limit is taken off a list**, which is the same
//! rule every other pair on this screen is decided by: a list is offered where the
//! choices are already in hand, and a line is opened where they are not. The steps an
//! age limit is offered as are a table compiled into this binary. The libraries are the
//! media server's, and reaching it is the one thing this screen does not do between a
//! keypress and the frame after it — so they are typed, the way the name of an archive
//! this screen is not holding is typed, and a name that matches no library is refused
//! by the core in the operator's own words, with the ones there are named.
//!
//! **The rows are not put to the translation first.** Every list built from names a
//! stack supplied offers each row to [`lemonfiber_api::actions::named`] and keeps what
//! comes back, because that table is the only thing that knows which of them an action
//! can carry. These rows are not names a stack supplied; they are the steps the core
//! offers, asked of the core, so there is no row here that might be refused — and the
//! words on each row are the core's too, which is what keeps them the same words a
//! household read says the limit back in.

use lemonfiber_core::age_limit;

use super::chooser::{Chooser, Listed};
use super::errand::{self, Errand, Given};
use super::{Press, Stage, Wanted};

/// What is asked above the line the libraries are typed on.
///
/// Naming none of them is the ordinary case and the one an operator who presses enter
/// lands on, so the line says so rather than leaving an empty answer to be guessed at.
pub(super) const ASKS_LIBRARIES: &str = "Which libraries, separated by commas; none is all of them";

/// What choosing no limit at all comes to, said on the row that opens the list.
///
/// Here rather than among the core's steps, because no limit is not a step among them:
/// it is the absence of one, and it is what this list opens on so that an operator who
/// presses enter through the errand sends the invitation this screen always sent.
const NO_LIMIT: &str = "they can watch anything in the libraries above";

/// What is said on the row that holds unrated content back.
///
/// The one an operator lands on by pressing enter, because it is what a restriction
/// defaults to everywhere else — and because the cost of the other answer is the one
/// nobody can weigh in advance.
const HOLD_UNRATED: &str = "safer; some legitimate content becomes invisible to them";

/// What is said on the row that lets it through.
const ALLOW_UNRATED: &str = "more permissive; content with no rating is unpredictable";

/// The word each row sends, as a request body and the command line both spell it.
///
/// The same two words the other surfaces take, so the choice made here is the choice
/// they make — a third spelling would be a third answer nobody could hold the others
/// to.
const HELD_BACK: &str = "block";

/// The word for letting it through.
const LET_THROUGH: &str = "allow";

/// One answer to what happens to content the media server has no rating for.
pub(super) struct Unrated {
    /// What it is called on the row.
    name: &'static str,
    /// What choosing it comes to, in the line beside the name.
    about: &'static str,
    /// What the errand is given by taking it.
    given: Given,
}

impl Listed for Unrated {
    fn name(&self) -> &str {
        self.name
    }

    fn about(&self) -> &str {
        self.about
    }
}

/// One answer to how far up the ratings an invitation goes.
pub(super) struct Limit {
    /// What it is called on the row, in the core's own words for that limit.
    name: String,
    /// What choosing it comes to, in the line beside the name.
    about: &'static str,
    /// What has been answered by the time this row is taken, kept because the
    /// question at the end says every answer together and the two lines they were
    /// given on are gone by then.
    answered: Answered,
}

/// What has been answered by the time an age limit is taken.
///
/// Carried on each row rather than beside the chooser, because the row is what a
/// keypress hands on and a second copy alongside it is a copy able to disagree with
/// the one that was actually taken.
struct Answered {
    /// Who the invitation is for.
    name: String,
    /// The libraries typed, as separate names. Empty is every one.
    libraries: Vec<String>,
    /// The limit the row carries, where the row carries one.
    age_limit: Option<u32>,
}

impl Listed for Limit {
    fn name(&self) -> &str {
        &self.name
    }

    fn about(&self) -> &str {
        self.about
    }
}

/// The line the libraries are typed on, the name having been typed.
pub(super) fn over(errand: &'static Errand, name: String) -> Stage {
    Stage::Allowing {
        errand,
        name,
        typed: String::new(),
    }
}

/// Over that line: type, take back, go on, or leave it.
pub(super) fn allowing(
    stage: &mut Stage,
    errand: &'static Errand,
    name: String,
    mut typed: String,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => {
            *stage = limiting(errand, &name, &named(&typed));
            return Wanted::Nothing;
        }
        Press::Rubout => {
            typed.pop();
        }
        Press::Typed(character) => typed.push(character),
        Press::Back | Press::Forward => (),
    }
    *stage = Stage::Allowing {
        errand,
        name,
        typed,
    };
    Wanted::Nothing
}

/// The libraries a line was typed with, as separate names.
///
/// Split on commas because that is how the line asks for them, and the empty pieces a
/// trailing comma leaves are dropped: a name that is nothing at all would be sent to
/// the core to be refused for matching no library, which is a true sentence about a
/// library nobody named.
fn named(typed: &str) -> Vec<String> {
    typed
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The list of how far up the ratings this invitation goes, over what was typed.
///
/// No limit first, because it is the ordinary answer and the one an operator who
/// presses enter through this errand lands on. The core's own steps follow in its own
/// order, said in its own words — so a step added there is on this list, and reads the
/// same as a household list reads the same limit back, without anybody editing this
/// file.
fn limiting(errand: &'static Errand, name: &str, libraries: &[String]) -> Stage {
    let row = |age: Option<u32>, suits: &'static str| Limit {
        name: age_limit::reading(age),
        about: suits,
        answered: Answered {
            name: name.to_owned(),
            libraries: libraries.to_vec(),
            age_limit: age,
        },
    };
    Stage::Limiting {
        errand,
        chooser: Chooser::over(
            row(None, NO_LIMIT),
            age_limit::steps()
                .iter()
                .map(|step| row(Some(step.age), step.suits))
                .collect(),
        ),
    }
}

/// Over how far up the ratings it goes: move, take one, or leave it.
pub(super) fn limited(
    stage: &mut Stage,
    errand: &'static Errand,
    mut chooser: Chooser<Limit>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => {
            let answered = chooser.taken().answered;
            // Nothing narrowed is nothing written, so there is no unrated content to
            // decide about: the errand goes straight to its question, saying nothing
            // about a setting this run does not touch.
            if answered.age_limit.is_none() && answered.libraries.is_empty() {
                let given = Given::inviting(&answered.name, answered.libraries, None, None);
                return errand::begun(stage, errand, given);
            }
            *stage = unrating(errand, &answered);
            return Wanted::Nothing;
        }
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Limiting { errand, chooser };
    Wanted::Nothing
}

/// The list of what happens to content the media server has no rating for.
///
/// Held back first, because it is what a restriction defaults to on every other
/// surface and an operator pressing enter through this errand should land on the same
/// answer a command line leaving the flag out lands on.
fn unrating(errand: &'static Errand, answered: &Answered) -> Stage {
    let row = |word: &'static str, name: &'static str, about: &'static str| Unrated {
        name,
        about,
        given: Given::inviting(
            &answered.name,
            answered.libraries.clone(),
            answered.age_limit,
            // The row's own name is what the question says it as, so the sentence an
            // operator agrees to and the row they took cannot come apart.
            Some((word, name)),
        ),
    };
    Stage::Unrated {
        errand,
        chooser: Chooser::over(
            row(HELD_BACK, "nothing unrated", HOLD_UNRATED),
            vec![row(
                LET_THROUGH,
                "including what has no rating",
                ALLOW_UNRATED,
            )],
        ),
    }
}

/// Over what happens to unrated content: move, take one, or leave it.
pub(super) fn unrated(
    stage: &mut Stage,
    errand: &'static Errand,
    mut chooser: Chooser<Unrated>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return errand::begun(stage, errand, chooser.taken().given),
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Unrated { errand, chooser };
    Wanted::Nothing
}

#[cfg(test)]
mod tests {
    use super::{
        allowing, limited, named, over, unrated, ALLOW_UNRATED, ASKS_LIBRARIES, HELD_BACK,
        HOLD_UNRATED, LET_THROUGH, NO_LIMIT,
    };
    use crate::acting::errand::{self, Errand};
    use crate::acting::{Press, Stage};
    use lemonfiber_api::actions::Arguments;
    use lemonfiber_core::age_limit;

    /// The errand these two stages belong to, taken off the list every surface reads
    /// it from rather than built here: a fixture of its own would let this file pass
    /// while the screen offered something else.
    fn inviting() -> &'static Errand {
        errand::tests::sending("invite").unwrap_or(errand::all().0)
    }

    /// One press over whichever half is open, leaving any other stage alone.
    ///
    /// The two halves are driven through the same door a keypress arrives at rather
    /// than by calling into either, because what is being tested is a sequence: the
    /// name is typed, the line is gone by the time the list is shown, and the question
    /// at the end has to say all three answers.
    fn press(stage: &mut Stage, pressed: &Press) {
        match std::mem::replace(stage, Stage::Idle) {
            Stage::Allowing {
                errand,
                name,
                typed,
            } => {
                let _ = allowing(stage, errand, name, typed, pressed);
            }
            Stage::Limiting { errand, chooser } => {
                let _ = limited(stage, errand, chooser, pressed);
            }
            Stage::Unrated { errand, chooser } => {
                let _ = unrated(stage, errand, chooser, pressed);
            }
            other => *stage = other,
        }
    }

    /// Type a word, one character at a time.
    fn typing(stage: &mut Stage, word: &str) {
        for character in word.chars() {
            press(stage, &Press::Typed(character));
        }
    }

    /// What the question at the end was given, and what it says — empty where the
    /// errand reached no question, which fails whatever the assertion was.
    fn agreed(stage: &Stage) -> (Arguments, String) {
        match stage {
            Stage::Agreeing { given, .. } => (given.asked(), given.said().to_owned()),
            _ => (Arguments::default(), String::new()),
        }
    }

    /// A stage that reached no question is read as having been given nothing.
    ///
    /// The reader above says so, and every assertion below leans on it: a stage that
    /// stopped early has to come back empty rather than carrying whatever the last
    /// answered question left, or a test would pass on an errand that never asked.
    #[test]
    fn a_stage_that_asked_nothing_is_read_as_empty() {
        let (asked, said) = agreed(&Stage::Idle);

        assert_eq!(asked, Arguments::default());
        assert!(said.is_empty(), "{said}");
    }

    /// A line of library names becomes the names, with the spaces and the empty
    /// pieces a stray comma leaves dropped.
    #[test]
    fn a_typed_line_becomes_the_libraries_it_named() {
        assert_eq!(
            named(" Films , Shows ,"),
            vec!["Films".to_owned(), "Shows".to_owned()]
        );
    }

    /// An empty line names no library, which is every one rather than none.
    #[test]
    fn an_empty_line_names_no_library() {
        assert!(named("").is_empty());
        assert!(named("  ,  ").is_empty());
    }

    /// The rows offered are no limit and the core's own steps, said in the core's own
    /// words, so a step added there is on this list without anybody editing it.
    #[test]
    fn the_rows_are_no_limit_and_the_steps_the_core_offers() {
        assert!(
            age_limit::steps().len() > 1,
            "the core offers no ladder to read"
        );
        assert!(
            !NO_LIMIT.is_empty(),
            "the row that opens the list says nothing about what taking it comes to"
        );
        assert!(
            ASKS_LIBRARIES.contains("none is all of them"),
            "the line does not say what naming none comes to"
        );
    }

    /// The libraries typed and the age limit taken off the list reach the question
    /// together, as one sentence saying all three answers.
    ///
    /// What is under test is that the second answer does not lose the first: the line
    /// the name was typed on is gone by the time the list is on the screen.
    #[test]
    fn what_was_typed_and_what_was_taken_reach_the_question_together() {
        let mut stage = over(inviting(), "ana".to_owned());

        typing(&mut stage, "Films, Shows");
        press(&mut stage, &Press::Accept);
        // The row under no limit, which is the lowest step the core offers.
        press(&mut stage, &Press::Forward);
        press(&mut stage, &Press::Accept);
        // Something was narrowed, so the third question is asked; the row it opens on
        // is the one a restriction defaults to everywhere else.
        press(&mut stage, &Press::Accept);

        let (asked, said) = agreed(&stage);
        assert_eq!(asked.name.as_deref(), Some("ana"), "{said}");
        assert_eq!(asked.libraries, ["Films".to_owned(), "Shows".to_owned()]);
        assert_eq!(
            asked.age_limit,
            age_limit::steps().first().map(|step| step.age),
            "the row under no limit did not become the youngest step the core offers"
        );
        assert!(
            said.contains("ana") && said.contains("Films, Shows"),
            "the question did not say what it was given: {said}"
        );
    }

    /// Pressing enter through the errand chooses the ordinary case — every library and
    /// no age limit, which is the invitation this screen always sent.
    #[test]
    fn pressing_enter_through_it_chooses_every_library_and_no_limit() {
        let mut stage = over(inviting(), "ana".to_owned());

        press(&mut stage, &Press::Accept);
        press(&mut stage, &Press::Accept);

        let (asked, said) = agreed(&stage);
        assert!(asked.libraries.is_empty(), "{:?}", asked.libraries);
        assert_eq!(asked.age_limit, None, "{said}");
        assert!(
            said.contains("everything"),
            "the question did not say that naming none is all of them: {said}"
        );
    }

    /// Content the media server has no rating for is asked about last, and only where
    /// the answers before it narrowed something.
    ///
    /// An offer that names neither a library nor a limit writes no policy at all, so
    /// the question would be about a setting the run does not touch — and a keypress
    /// the ordinary case does not owe.
    #[test]
    fn what_happens_to_unrated_content_is_asked_only_where_something_was_narrowed() {
        let mut stage = over(inviting(), "ana".to_owned());
        press(&mut stage, &Press::Accept);
        press(&mut stage, &Press::Accept);
        assert!(
            matches!(&stage, Stage::Agreeing { .. }),
            "an offer that narrowed nothing was asked about unrated content"
        );

        let mut stage = over(inviting(), "ana".to_owned());
        typing(&mut stage, "Films");
        press(&mut stage, &Press::Accept);
        press(&mut stage, &Press::Accept);
        assert!(
            matches!(&stage, Stage::Unrated { .. }),
            "an offer that narrowed a library was not asked about unrated content"
        );
    }

    /// The two answers reach the question, and the row it opens on is the one a
    /// restriction defaults to on every other surface.
    #[test]
    fn the_answer_about_unrated_content_reaches_the_question_with_the_rest() {
        for (presses, expected) in [(0, HELD_BACK.to_owned()), (1, LET_THROUGH.to_owned())] {
            let mut stage = over(inviting(), "ana".to_owned());
            typing(&mut stage, "Films");
            press(&mut stage, &Press::Accept);
            press(&mut stage, &Press::Accept);
            for _ in 0..presses {
                press(&mut stage, &Press::Forward);
            }
            press(&mut stage, &Press::Accept);

            let (asked, said) = agreed(&stage);
            assert_eq!(asked.name.as_deref(), Some("ana"), "{said}");
            assert_eq!(asked.libraries, ["Films".to_owned()], "{said}");
            // Both rows send a word. The answer is a choice with a cost either way,
            // so the row an operator lands on is an answer they gave rather than one
            // this screen left for somewhere else to fill in.
            assert_eq!(asked.unrated.as_deref(), Some(expected.as_str()), "{said}");
        }
    }

    /// Both rows say what taking them comes to, because either answer has a cost.
    #[test]
    fn both_answers_about_unrated_content_say_what_they_cost() {
        assert!(HOLD_UNRATED.contains("invisible"), "{HOLD_UNRATED}");
        assert!(ALLOW_UNRATED.contains("unpredictable"), "{ALLOW_UNRATED}");
    }

    /// Backing out of the last question closes the errand rather than sending it with
    /// an answer nobody gave.
    #[test]
    fn backing_out_of_the_last_question_closes_it() {
        let mut stage = over(inviting(), "ana".to_owned());
        typing(&mut stage, "Films");
        press(&mut stage, &Press::Accept);
        press(&mut stage, &Press::Accept);
        press(&mut stage, &Press::Abandon);

        assert!(matches!(stage, Stage::Idle), "the last list did not close");
    }

    /// The last list answers no keypress it has no use for.
    #[test]
    fn the_last_list_answers_no_press_it_has_no_use_for() {
        let mut stage = over(inviting(), "ana".to_owned());
        typing(&mut stage, "Films");
        press(&mut stage, &Press::Accept);
        press(&mut stage, &Press::Accept);
        press(&mut stage, &Press::Typed('x'));
        press(&mut stage, &Press::Rubout);
        press(&mut stage, &Press::Forward);
        press(&mut stage, &Press::Back);
        press(&mut stage, &Press::Accept);

        let (asked, said) = agreed(&stage);
        assert_eq!(
            asked.unrated.as_deref(),
            Some(HELD_BACK),
            "typing at the last list moved it: {said}"
        );
    }

    /// Taking a character back takes one character back, and moving down the list of
    /// age limits and back up comes back to where it opened.
    #[test]
    fn a_line_is_corrected_and_a_list_moves_both_ways() {
        let mut stage = over(inviting(), "ana".to_owned());

        typing(&mut stage, "Filmsx");
        press(&mut stage, &Press::Rubout);
        assert!(
            matches!(&stage, Stage::Allowing { typed, .. } if typed == "Films"),
            "the line did not take a character back"
        );

        press(&mut stage, &Press::Accept);
        press(&mut stage, &Press::Forward);
        press(&mut stage, &Press::Back);
        press(&mut stage, &Press::Accept);
        // A library was named, so the third question stands between the list and the
        // question at the end.
        press(&mut stage, &Press::Accept);

        let (asked, said) = agreed(&stage);
        assert!(
            matches!(&stage, Stage::Agreeing { .. }),
            "the errand did not reach its question: {said}"
        );
        assert_eq!(
            asked.age_limit, None,
            "moving down the list and back up did not come back to no limit: {said}"
        );
    }

    /// Backing out of either half closes the errand rather than going on with half an
    /// answer.
    #[test]
    fn backing_out_of_either_half_closes_it() {
        let mut stage = over(inviting(), "ana".to_owned());
        typing(&mut stage, "Films");
        press(&mut stage, &Press::Abandon);
        assert!(matches!(stage, Stage::Idle), "the line did not close");

        let mut stage = over(inviting(), "ana".to_owned());
        press(&mut stage, &Press::Accept);
        press(&mut stage, &Press::Abandon);
        assert!(matches!(stage, Stage::Idle), "the list did not close");
    }

    /// Neither half answers a keypress it has no use for, and neither answers one that
    /// arrives after the errand has moved on.
    #[test]
    fn neither_half_answers_a_press_it_has_no_use_for() {
        let mut stage = over(inviting(), "ana".to_owned());

        press(&mut stage, &Press::Forward);
        assert!(
            matches!(&stage, Stage::Allowing { typed, .. } if typed.is_empty()),
            "moving over a line changed it"
        );

        press(&mut stage, &Press::Accept);
        press(&mut stage, &Press::Typed('x'));
        press(&mut stage, &Press::Rubout);
        press(&mut stage, &Press::Accept);
        let (asked, said) = agreed(&stage);
        assert_eq!(
            asked.age_limit, None,
            "typing at the list of age limits moved it: {said}"
        );

        // The question the errand has reached belongs to the flow next door, so a
        // press arriving here now is not either half's to answer.
        press(&mut stage, &Press::Accept);
        assert!(
            matches!(&stage, Stage::Agreeing { .. }),
            "a press after the errand moved on was answered here"
        );
    }
}
