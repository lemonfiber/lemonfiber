//! Naming the services inside what an action was already given.
//!
//! Four of this screen's requests take a service and had no way to be given one.
//! Three of them are lifecycle actions on keys of their own — starting, stopping and
//! restarting some of a form's services rather than the whole form — and the fourth
//! is the capture behind the other list, which takes the one service whose
//! configuration goes into the archive. The command line spells all four
//! `--service`; the list this screen offered was the stack's own forms, and the
//! services inside one were a gather it did not have.
//!
//! **It has the gather.** The panel this box is drawn over lists every service and
//! what it is doing, refreshed every second, taken from the manifest rather than
//! from what happens to be running — so a service that has never started is on it.
//! The names are therefore in hand at the moment the key is pressed, which is the
//! rule the narrowing next door already follows: pick where the list is already
//! there, and type only where fetching it would cost the very request being narrowed
//! away from. Here it would cost more than that. A read asked between a keypress and
//! the frame after it is awaited in the loop with nothing drawn, and this one reaches
//! the container engine — so a line to type a name on would have been the only other
//! option, and a typed service name is a name nothing checked before the work ran.
//!
//! **One list, whatever it is filling.** `up` and `down` reach a different command
//! when services are named — `Command::Start` beside `Command::Up`, the way Compose
//! spells the pair — while a restart and a capture carry the services as an argument
//! to the command they already reach. None of that is visible here, because nothing
//! here assembles a command: every row goes through
//! [`lemonfiber_api::actions::named`] and comes back as whatever that action reaches
//! given what it now has. The fork is the table's, and a screen that had to know
//! about it would be a screen keeping a second copy of the table.
//!
//! What does differ is one name against a list of them, and that is the table's
//! answer too — a capture's scope is one scope, so its rows carry no box to mark and
//! enter takes the row under the cursor.
//!
//! **Naming no service is going on with what was already named.** Every one of the
//! four reads an empty list as the whole of what it was given, so the row that says
//! so is offered on all four — unlike the list of forms, where three actions refuse
//! an empty one and the row is dropped for them. An action taking that row goes on
//! with the question it would have been put without this list at all, so the forms it
//! names are named by their own names rather than by a row saying every service.

use lemonfiber_api::actions::{Arguments, TAKES_SERVICE};
use lemonfiber_core::dashboard::Panel;
use lemonfiber_core::docker::Service;

use super::chooser::Chooser;
use super::errand::{self, Errand, Given};
use super::offer::{choices, or_refused, over, Choice, Fills, Offer, Over, Taken};
use super::{Press, Stage, Wanted};

/// What the row naming no service is called on an action's list, and what taking it
/// comes to.
///
/// Its own wording rather than the errand's, because the row means a different whole
/// under each. An action arrives having already been given its forms, so what it
/// declines to narrow is those; the errand arrives having been given nothing, so what
/// it declines to narrow is the stack.
const ALL_OF_THEM: (&str, &str) = (
    "all of them",
    "every service in what was named, rather than some of them",
);

/// The same row on the capture's list, which has narrowed nothing yet.
const WHOLE_STACK: (&str, &str) = (
    "the whole stack",
    "every service's configuration, rather than one service's",
);

/// What a list of services is being named for, and what naming them leads to.
///
/// The two flows that reach this list, held as what each needs to go on with rather
/// than as a flag: an action arrives having already been given its forms, and the
/// capture arrives having been given nothing.
pub(super) enum Inside {
    /// One of the actions on a key of its own, and what its list of forms came to.
    Action {
        /// The action being named services for.
        offer: &'static Offer,
        /// What its list of forms came to, which is what goes ahead where no service
        /// is named.
        taken: Taken,
    },
    /// The errand that captures one service's configuration.
    Errand(&'static Errand),
}

impl Inside {
    /// The action every surface calls this by.
    const fn action(&self) -> &'static str {
        match *self {
            Self::Action { offer, .. } => offer.action,
            Self::Errand(errand) => errand.action,
        }
    }

    /// What was named before this list, which the services are named beside.
    fn before(&self) -> Arguments {
        match *self {
            Self::Action { ref taken, .. } => beside(taken),
            Self::Errand(_) => Arguments::default(),
        }
    }

    /// What follows the list, once something has been taken off it.
    fn onwards(self, stage: &mut Stage, inside: Taken) -> Wanted {
        match self {
            Self::Action { offer, taken } => {
                *stage = Stage::Confirming {
                    offer,
                    taken: went_on(taken, inside),
                };
                Wanted::Nothing
            }
            Self::Errand(errand) => errand::begun(stage, errand, Given::picked(&inside)),
        }
    }
}

/// The forms an action was already given, as the argument a service is named beside.
fn beside(taken: &Taken) -> Arguments {
    Arguments {
        forms: taken.named(),
        ..Arguments::default()
    }
}

/// Which of the arguments this action's services go in.
///
/// Asked of the table that publishes both rather than decided again here. The command
/// line spells them alike and what tells them apart is that an archive records one
/// scope, which is that table's fact about the command rather than this screen's
/// about its list.
fn fills(action: &str) -> Fills {
    if TAKES_SERVICE.contains(&action) {
        Fills::Service
    } else {
        Fills::Services
    }
}

/// What an action goes on to be asked about: the services where any were named, and
/// what it was already given where none were.
///
/// The row naming no service comes to the same command the list before it came to, so
/// what is left to decide is which words the question is put in. Naming nothing
/// narrows nothing, so it is put in the words it would have been put in without this
/// list at all — the forms by their own names, rather than a row saying every service.
fn went_on(before: Taken, inside: Taken) -> Taken {
    if inside.covers.iter().all(|choice| choice.names.is_empty()) {
        before
    } else {
        inside
    }
}

/// The services the screen has in hand, as the rows a list is built from.
///
/// Nothing at all where the panel could not be filled: a stack the engine cannot be
/// asked about has no services to narrow to.
pub(super) fn gathered(panel: &Panel<Vec<Service>>) -> Vec<(String, String, String)> {
    let Panel::Ready(services) = panel else {
        return Vec::new();
    };
    services
        .iter()
        .map(|service| {
            (
                service.id.clone(),
                service.name.clone(),
                crate::render::stack::doing(service),
            )
        })
        .collect()
}

/// The list of services this action could be given, or the question itself where it
/// could be given none.
pub(super) fn or_the_question(
    offer: &'static Offer,
    taken: Taken,
    gathered: &[(String, String, String)],
) -> Stage {
    match offered(offer.action, ALL_OF_THEM, &beside(&taken), gathered) {
        Some(chooser) => Stage::Inside {
            inside: Inside::Action { offer, taken },
            chooser,
        },
        None => Stage::Confirming { offer, taken },
    }
}

/// What a capture is given where there was no service to choose between.
pub(super) fn nothing_to_choose() -> Given {
    Given::whole(WHOLE_STACK.0)
}

/// The list of services this errand could be given, or nothing where there are none.
pub(super) fn for_the_errand(
    errand: &'static Errand,
    gathered: &[(String, String, String)],
) -> Option<Stage> {
    offered(errand.action, WHOLE_STACK, &Arguments::default(), gathered).map(|chooser| {
        Stage::Inside {
            inside: Inside::Errand(errand),
            chooser,
        }
    })
}

/// The list one of these could be given, or nothing where it could be given none.
///
/// Every service goes through the translation and only what comes to a command is
/// offered, which is [`super::offer::choices`]'s rule rather than a second one. What
/// survives is then held to one thing: a list has to offer more than the row that
/// names nothing, or it is not a choice.
///
/// That one rule answers both ways this list can come to nothing. An action with
/// nowhere to put a service has every service row refused, and a screen that could
/// not reach the container engine has no service row to refuse — and in each case the
/// flow goes on as though this list did not exist. Which actions those are is the
/// translation's answer rather than a list of names kept here.
fn offered(
    action: &str,
    whole: (&str, &str),
    before: &Arguments,
    gathered: &[(String, String, String)],
) -> Option<Chooser<Choice>> {
    let mut subjects = vec![(Vec::new(), whole.0.to_owned(), whole.1.to_owned())];
    subjects.extend(
        gathered
            .iter()
            .map(|(id, name, doing)| (vec![id.clone()], name.clone(), doing.clone())),
    );
    let (first, rest) = choices(action, fills(action), before, subjects).ok()?;
    (!rest.is_empty()).then(|| Chooser::over(first, rest))
}

/// Over the services inside what was named: move, mark, take, or leave it.
pub(super) fn choosing(
    stage: &mut Stage,
    inside: Inside,
    chooser: Chooser<Choice>,
    press: &Press,
) -> Wanted {
    let action = inside.action();
    match over(action, fills(action), &inside.before(), chooser, press) {
        Over::Left => Wanted::Nothing,
        Over::Choosing(chooser) => {
            *stage = Stage::Inside { inside, chooser };
            Wanted::Nothing
        }
        Over::Taken(taken) => match or_refused(stage, taken) {
            Some(taken) => inside.onwards(stage, taken),
            None => Wanted::Nothing,
        },
    }
}

#[cfg(test)]
pub(crate) mod tests;
