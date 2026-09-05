//! What a question is, apart from what the questions are.
//!
//! The vocabulary only: which of a read's arguments a narrowing fills, what a
//! question has to be given before it can be asked, and what one comes to once it
//! has been. Next door is the list itself and the flow over it, which stay together
//! for the reason that file gives — what a flow decides belongs beside the list it
//! decides over. What a question *is* is not part of that decision, and it was the
//! half that could leave without taking the reason with it.
//!
//! The two crossed the file cap together rather than either outgrowing it: one slice
//! added a question about what the household may ask for and another added one about
//! the line, and four lines was the difference. Split rather than raised, because the
//! cap is a limit on what can be read in one sitting, and two slices landing on one
//! file in a day does not make that file easier to read.

use lemonfiber_api::reads::{named, Wanted as Asking};
use lemonfiber_core::app::Command;

use crate::acting::chooser::Listed;

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
    ///
    /// Visible to the whole of `acting`, because `narrowing` reads it and is a
    /// sibling of `question` rather than a child of it.
    pub(in crate::acting) const fn picking(&self) -> Option<(&'static str, Narrows)> {
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
