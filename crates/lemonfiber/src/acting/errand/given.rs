//! What an errand has to be given before it can be sent, and what it was given.
//!
//! The vocabulary of one errand's subject, apart from the list and the question that
//! surround it. [`super`] decides which errands there are, what each says before it is
//! agreed to and what the yes sends; this is what "what it was given" means on the way
//! through.
//!
//! **One shape over every kind of subject.** A restore takes the name a backup was
//! written under, a capture takes one of the services the panels are showing, a bundle
//! takes how much log and what becomes of media filenames, letting a download go takes
//! the name a client and the disk account both use for it, and the rest take nothing at
//! all. What follows any of them is the same path — the run that says what the errand
//! would do, the question, and the errand itself — so what arrives at that path is one
//! value rather than one per subject, and the question is put in one voice rather than
//! in several that drifted apart.
//!
//! **What is sent and what is said travel together.** The arguments are what the
//! command carries and the sentence is what the operator agreed to, and they are built
//! in the same breath from the same answer. Kept apart they would eventually disagree,
//! and the place they would disagree is the line above the yes.

use lemonfiber_api::actions::Arguments;
use lemonfiber_core::bundle::Filenames;

use super::super::offer::Taken;

/// What an errand has to be given before it can be sent.
///
/// Which of the two shapes each takes is decided the way the questions on the other
/// list decide it: by where the thing being named already is. An archive is written
/// under a name nothing on this screen is holding, so it is typed. A service is on the
/// panel this box is drawn over, so it is taken off a list — and a typed service name
/// would be a name nothing checked before the capture ran.
pub(crate) enum Needs {
    /// Nothing: it is sent as it stands.
    Nothing,
    /// The name a backup was written under, typed on a line of its own, with what is
    /// asked for above it.
    Archive(&'static str),
    /// One of the services the screen has in hand, or the whole stack.
    Service,
    /// Somebody's name, typed on a line of its own.
    ///
    /// Typed rather than taken, and for the opposite reason a service is taken: the
    /// person being invited is not on this screen yet. That is the whole point of
    /// inviting them.
    Named(&'static str),
    /// Somebody's name, then what they may watch: the libraries, typed on a line of
    /// their own, and then the age limit, taken off a list.
    ///
    /// Three answers and two shapes, decided the way everything else on this screen is.
    /// The person and the libraries are typed because neither is on this screen — the
    /// person is the point of inviting them, and the libraries are the media server's
    /// and this screen does not reach it between one keypress and the next. The age
    /// limit is taken off a list because the steps it is offered as are a table
    /// compiled into this binary, which is a list already in hand.
    Invitation(&'static str),
    /// Which completed download, typed on a line of its own.
    ///
    /// Typed rather than taken, for the same reason an archive is: what a download
    /// client is holding is not on this screen. The panels show what is *arriving*,
    /// and this errand is about what has arrived and is still being given back.
    Download(&'static str),
    /// What a bundle is to hold: how much of each service's log, typed on a line of
    /// its own, and then what becomes of media filenames, taken off a list.
    ///
    /// Two answers and two shapes, decided the way everything else on this screen is:
    /// a window is a number and there is no list of numbers to take one off, and what
    /// happens to filenames is two values of an enum the command carries, which is a
    /// list already in hand.
    Bundling(&'static str),
}

/// What an errand was given, and what that is called where the question says it.
///
/// The arguments rather than the text they came from, because they come from two
/// places — a name typed on a line and a service taken off a list — and what follows
/// is one path over both: the run that says what the errand would do, the question,
/// and the errand itself. Two shapes reaching that path would be two accounts of what
/// an operator agreed to.
pub(crate) struct Given {
    /// What the errand is given, empty where it takes nothing.
    asked: Arguments,
    /// What that is called where the question has to say it, empty where the errand
    /// takes nothing and the question is whole without a subject.
    said: String,
}

impl Given {
    /// An errand that takes nothing, given nothing.
    pub(crate) fn nothing() -> Self {
        Self {
            asked: Arguments::default(),
            said: String::new(),
        }
    }

    /// The name of an archive, as it was typed.
    ///
    /// Nothing typed is nothing named, rather than an empty name. An empty one is a
    /// name the core would go and fail to find, which costs a round trip to be told
    /// what the translation already knows — the same reading a trace with nothing
    /// typed is given.
    pub(crate) fn typed(typed: String) -> Self {
        Self {
            asked: Arguments {
                archive: (!typed.is_empty()).then(|| typed.clone()),
                ..Arguments::default()
            },
            said: typed,
        }
    }

    /// Somebody's name, for an errand that is about a person rather than a file.
    ///
    /// The same line and the same emptiness rule as [`Given::typed`]; what differs is
    /// only which argument the word fills, which is the errand's business rather than
    /// the line's.
    pub(crate) fn named(typed: String) -> Self {
        Self {
            asked: Arguments {
                name: (!typed.is_empty()).then(|| typed.clone()),
                ..Arguments::default()
            },
            said: typed,
        }
    }

    /// The completed download to stop seeding, as it was typed.
    ///
    /// The same line and the same emptiness rule as [`Given::typed`]; what differs is
    /// only which argument the word fills. A download named nothing is refused by the
    /// translation rather than sent, which is the whole point of routing it through
    /// the same table a browser is answered from.
    pub(crate) fn downloaded(typed: String) -> Self {
        Self {
            asked: Arguments {
                download: (!typed.is_empty()).then(|| typed.clone()),
                ..Arguments::default()
            },
            said: typed,
        }
    }

    /// Somebody's name, what they may watch, and how far up the ratings they may go —
    /// the whole of what an invitation is given, once all three lines are answered.
    ///
    /// The sentence is built here beside the arguments for the reason every other one
    /// is: it is what the operator agrees to, and kept apart from what is sent the two
    /// would eventually disagree — in the line above the yes, which is the worst place
    /// for them to.
    ///
    /// The limit is said in the words a household read says the same limit back in,
    /// off [`lemonfiber_core::age_limit`], because it is one setting and two surfaces
    /// naming it differently is what that one place exists to prevent.
    /// `unrated` is the word the request carries beside what the question says it as,
    /// because they are not the same thing: `block` is how the other two surfaces spell
    /// the choice, and it is not a sentence anybody would agree to.
    pub(crate) fn inviting(
        name: &str,
        libraries: Vec<String>,
        age_limit: Option<u32>,
        unrated: Option<(&'static str, &'static str)>,
    ) -> Self {
        let watching = if libraries.is_empty() {
            "who can watch everything".to_owned()
        } else {
            format!("who can watch {}", libraries.join(", "))
        };
        let limited = age_limit.map_or_else(String::new, |age| {
            format!(", and {}", lemonfiber_core::age_limit::reading(Some(age)))
        });
        // Said only where it was asked. An offer that narrows nothing writes nothing,
        // so what would happen to unrated content is a question about an account
        // nobody is limiting — and a sentence answering it would be a promise about a
        // setting this run does not touch.
        let unrated_said = unrated.map_or_else(String::new, |(_, said)| format!(", {said}"));
        Self {
            asked: Arguments {
                name: (!name.is_empty()).then(|| name.to_owned()),
                libraries,
                age_limit,
                unrated: unrated.map(|(word, _)| word.to_owned()),
                ..Arguments::default()
            },
            said: format!("{name}, {watching}{limited}{unrated_said}"),
        }
    }

    /// The whole of what the errand is about, where there was nothing to narrow it to.
    ///
    /// Said rather than left out. A capture with no service to choose between is the
    /// whole stack, and the question it is put reads as a sentence with its subject
    /// missing if nothing says so — which is exactly the screen an operator gets when
    /// the container engine could not be reached.
    pub(crate) fn whole(said: &str) -> Self {
        Self {
            asked: Arguments::default(),
            said: said.to_owned(),
        }
    }

    /// The service that was taken off the list, or the whole stack where the row
    /// naming none was.
    pub(crate) fn picked(taken: &Taken) -> Self {
        Self {
            asked: Arguments {
                service: taken.named().into_iter().next(),
                ..Arguments::default()
            },
            said: taken.name(),
        }
    }

    /// What a bundle is to hold, once both halves have been answered.
    ///
    /// The window is carried as a number rather than left out, so what the question
    /// says and what the command is given are one figure. Left out, the core would
    /// supply its own and the sentence above the question would be this screen's
    /// guess at what that is.
    pub(crate) fn bundled(lines: u32, filenames: Filenames) -> Self {
        let shown = match filenames {
            Filenames::Replaced => "media filenames replaced",
            Filenames::Shown => "media filenames shown as they are",
        };
        Self {
            asked: Arguments {
                logs: Some(lines),
                filenames,
                ..Arguments::default()
            },
            said: format!("with the last {lines} lines of each service's log, and {shown}"),
        }
    }

    /// The same, carrying the name the offer this screen just read gave itself.
    ///
    /// The screen holds the question open in the process that asked it, so the name is
    /// carried rather than typed — which is exactly what a browser cannot do, and why
    /// the name exists at all. What is agreed to is still the reading that was shown:
    /// this carries the offer that was read, not a standing yes to whatever stands.
    pub(super) fn answering(mut self, offer: String) -> Self {
        self.asked.offer = Some(offer);
        self
    }

    /// The same, having accepted the re-point the archive's own account called for.
    ///
    /// The acceptance goes on after the account and never before it. Whether there is
    /// anything to accept is the core's to know — it is the archive that says which
    /// data root it was taken against — so a question asked ahead of the listing would
    /// be asking somebody to agree to a move that may not be happening.
    pub(super) fn repointing(mut self, accepts: &'static str) -> Self {
        self.asked.repoint = true;
        self.said = format!("{}, {accepts}", self.said);
        self
    }

    /// What the question calls it.
    pub(crate) fn said(&self) -> &str {
        &self.said
    }

    /// The arguments it fills, as the errand's own runs are given them.
    ///
    /// Handed over rather than read off a field, so what an errand is given stays here
    /// and what is done with it — the run that only reports, the run that acts — stays
    /// next door.
    pub(crate) fn asked(&self) -> Arguments {
        self.asked.clone()
    }
}
