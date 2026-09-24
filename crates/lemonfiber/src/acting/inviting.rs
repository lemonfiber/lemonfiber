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
mod tests;
