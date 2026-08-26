//! The two requests this screen starts and then watches, rather than waits for.
//!
//! An errand behind the other list is agreed to, run, and read: the screen has
//! nothing to say between the yes and the answer, because the answer is the whole
//! of what the errand produces. These two are not that shape. A walk runs for
//! minutes and says each step the moment it is true — a walk read back afterwards
//! is a report, and the operator would have learned what happened rather than
//! watched it happen. A guard says nothing at all and has no ending of its own: it
//! holds until the data location is lost, which on a machine where the drive stays
//! put is never.
//!
//! So they are behind a key of their own rather than on the list of errands, and
//! the key says what the two have in common and nothing more: they keep going.
//!
//! **Both reach the action every surface calls them by.** A walk goes through
//! [`lemonfiber_api::actions::named`] as `walkthrough` and a guard as `watch`,
//! exactly as the five on their own keys and the six on the other list do. What a
//! guard may be given is that table's answer too: it refuses an empty list of
//! forms, so the whole stack is not among the choices offered here — that is
//! [`super::offer::choices`] dropping what the translation refuses, not a second
//! rule written down at this screen.
//!
//! **A walk's words are the core's.** Each step arrives as a
//! [`lemonfiber_core::walkthrough::Line`] and is put on the screen by
//! [`crate::render::walkthrough::spoken`] — the same function the same walk typed at
//! a shell is drawn by, and the same rule about which steps are worth saying aloud.
//! A copy of the walk's prose beside the core's would be two accounts of one run.
//!
//! **Only the one with no ending of its own is offered an end.** Which one that is
//! is asked of [`lemonfiber_api::jobs::leased`] rather than decided again here: the
//! web holds a guard's name on a lease and lets it go when nobody is asking, because
//! a browser has no interruption to send. A terminal does, and on this screen it is
//! not a signal — the terminal is in raw mode — so it is free to mean here what it
//! means in a shell. Everything else ends by itself, so leaving is all the screen
//! needs to offer and the run waits for it, which is what the other three flows
//! already do.

use lemonfiber_api::actions::{named, Arguments};
use lemonfiber_api::jobs::{leased, Lease};
use lemonfiber_core::app::Command;
use lemonfiber_core::walkthrough::Line as Step;

use super::chooser::{Chooser, Listed};
use super::offer::{Choice, Over, Taken};
use super::reading::{moved, Reading};
use super::{Asked, Press, Stage, Wanted};
use crate::render::walkthrough::{is_worth_saying, spoken};

/// The key that opens the two that keep going.
pub(crate) const KEY: char = 'k';

/// The word the footer puts beside that key.
pub(crate) const HINT: &str = "keeps going";

/// The question a walk that was told to look for nothing is put.
///
/// Its own sentence rather than the ordinary one left unfinished: naming nothing is a
/// request in its own right — a walk asked for nothing in particular suggests
/// something likely to work, which is what an operator with an empty library needs —
/// and "walk through?" asks nothing at all.
pub(super) const ANYTHING: &str = "Find something this stack can already fetch, and walk \
                                   through that";

/// What is said once a guard has been stopped from this screen.
///
/// What the interruption leaves behind is said rather than left to be inferred: a
/// guard that is let go stops guarding and stops nothing else, and an operator who
/// read only "stopped" could reasonably think their services had been.
const LET_GO: &str =
    "The guard has been let go. Nothing was stopped with it — the services are as they were.";

/// What one of these is given before it starts, and what it says while it runs.
///
/// One variant per request rather than two questions asked separately, because the
/// two halves go together: the one that is told what to look for is the one that
/// says what it is doing, and the one chosen a form is the one that says nothing.
enum Going {
    /// A walk: told on a line of its own what to look for, and saying each step the
    /// moment it is true. What it is asked for is above that line.
    Walking(&'static str),
    /// A guard: given one of the stack's own forms, and saying nothing at all until
    /// the location under it is lost.
    Guarding,
}

/// One request this screen starts and then watches.
pub(crate) struct Lasting {
    /// What it is called on the list, and on the box while it runs.
    pub(crate) name: &'static str,
    /// What it does, in one line.
    pub(crate) about: &'static str,
    /// The name every surface calls this action by.
    pub(crate) action: &'static str,
    /// How the question before it begins, what it was given completing it.
    pub(crate) asks: &'static str,
    /// What it is given, and what it says while it runs.
    going: Going,
}

/// The one the list opens on.
///
/// Held apart from the rest for the reason the selected errand and the selected
/// question are: a list built from a slice that might have been empty carries a case
/// for there being nothing to choose, which is not a state this screen can be in.
static OPENS_ON: Lasting = Lasting {
    name: "a walk through",
    about: "find one thing, fetch it, and say each step as it becomes true",
    action: "walkthrough",
    asks: "Walk through",
    going: Going::Walking("What to look for, or nothing at all to be suggested something"),
};

/// The rest of them, which is one.
static AFTER: &[Lasting] = &[Lasting {
    name: "a guard on the data location",
    about: "stop the services if the drive under them is disconnected",
    action: "watch",
    asks: "Guard the data location while running",
    going: Going::Guarding,
}];

impl Listed for Lasting {
    fn name(&self) -> &str {
        self.name
    }

    fn about(&self) -> &str {
        self.about
    }
}

/// What one of these was given, once it has been.
pub(super) enum Begun {
    /// What a walk was asked to look for, which is empty where nothing was named.
    Looked(String),
    /// The forms a guard was chosen for, and what guarding them comes to.
    Chosen(Taken),
}

/// The two, the one the list opens on apart from the rest.
pub(super) fn all() -> (&'static Lasting, Vec<&'static Lasting>) {
    (&OPENS_ON, AFTER.iter().collect())
}

/// Every one of them, in the order they are read.
#[cfg(test)]
pub(super) fn every() -> impl Iterator<Item = &'static Lasting> {
    std::iter::once(&OPENS_ON).chain(AFTER)
}

/// Over the list: move, take one, or leave it.
pub(super) fn starting(
    stage: &mut Stage,
    asked: &mut Option<Asked>,
    mut chooser: Chooser<&'static Lasting>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return taken(stage, asked, chooser.taken()),
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Starting(chooser);
    Wanted::Nothing
}

/// Take the one selected: open the line a walk is told what to look for on, or ask
/// the stack what a guard could be given.
///
/// The forms are asked for rather than remembered, for the reason an action's
/// subjects are: a stack's declarations are a file on disk an operator may have just
/// edited, and a list gathered once would offer a form that is no longer there.
fn taken(stage: &mut Stage, asked: &mut Option<Asked>, lasting: &'static Lasting) -> Wanted {
    match lasting.going {
        Going::Walking(asks) => {
            *stage = Stage::Wording {
                lasting,
                asks,
                typed: String::new(),
            };
            Wanted::Nothing
        }
        Going::Guarding => {
            *asked = Some(Asked::Guard(lasting));
            Wanted::Ask(Command::Forms)
        }
    }
}

/// Over the line being typed: type, take back, go on, or leave it.
///
/// Going on with nothing typed is a request rather than a mistake — a walk asked for
/// nothing in particular suggests something likely to work, which is what an operator
/// with an empty library needs — so the line is not held against being empty.
pub(super) fn wording(
    stage: &mut Stage,
    lasting: &'static Lasting,
    asks: &'static str,
    mut typed: String,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => {
            *stage = Stage::Beginning {
                lasting,
                begun: Begun::Looked(typed),
            };
            return Wanted::Nothing;
        }
        Press::Rubout => {
            typed.pop();
        }
        Press::Typed(character) => typed.push(character),
        Press::Back | Press::Forward => (),
    }
    *stage = Stage::Wording {
        lasting,
        asks,
        typed,
    };
    Wanted::Nothing
}

/// Over the forms a guard could be given: move, mark, take, or leave it.
///
/// The same movement an action's own subjects are chosen with, because it is the same
/// list — the stack's own forms, offered to the same translation. A guard named
/// several is one guard over several forms, which is what the command line has always
/// taken and what a browser sends whole.
pub(super) fn picking(
    stage: &mut Stage,
    lasting: &'static Lasting,
    chooser: Chooser<Choice>,
    press: &Press,
) -> Wanted {
    match super::offer::over(lasting.action, chooser, press) {
        Over::Left => (),
        Over::Choosing(chooser) => *stage = Stage::Picking { lasting, chooser },
        Over::Taken(taken) => {
            if let Some(taken) = super::offer::or_refused(stage, taken) {
                *stage = Stage::Beginning {
                    lasting,
                    begun: Begun::Chosen(taken),
                };
            }
        }
    }
    Wanted::Nothing
}

/// At the question: only an explicit yes goes ahead.
///
/// The same reading the teardown's own question and every errand are put under. On a
/// screen where one finger reaches a walk that will fetch a file, the answer that
/// changes something should never be the one given by accident.
pub(super) fn beginning(
    stage: &mut Stage,
    lasting: &'static Lasting,
    begun: Begun,
    press: &Press,
) -> Wanted {
    if !matches!(*press, Press::Typed('y' | 'Y')) {
        return Wanted::Nothing;
    }
    match begun {
        Begun::Chosen(taken) => {
            let command = taken.command.clone();
            keep(stage, lasting, taken.name(), command)
        }
        Begun::Looked(typed) => match named(lasting.action, looking(&typed)) {
            Ok(command) => keep(stage, lasting, typed, command),
            Err(no) => {
                *stage = Stage::Came(Reading::of(vec![no.said()]));
                Wanted::Nothing
            }
        },
    }
}

/// What a walk is given: the one thing to look for, or nothing named at all.
///
/// Nothing typed is nothing named rather than an empty title, which is the same
/// reading the translation gives a browser that sent the field and left it alone.
fn looking(typed: &str) -> Arguments {
    Arguments {
        item: (!typed.trim().is_empty()).then(|| typed.to_owned()),
        ..Arguments::default()
    }
}

/// Send it, and hold the screen where it can be watched.
///
/// Whether the screen offers to end it is the web's own answer for the same command,
/// asked here rather than decided again: the one held on a lease over there is the
/// one with no ending of its own, and it is the one that needs an end over here.
fn keep(stage: &mut Stage, lasting: &'static Lasting, named: String, command: Command) -> Wanted {
    *stage = Stage::Keeping {
        lasting,
        named,
        ends: matches!(leased(&command), Lease::WhileAsked),
        said: matches!(lasting.going, Going::Walking(_)).then(|| Reading::of(Vec::new())),
    };
    Wanted::Carry(command)
}

/// While it is running: move through what it has said, end it, or leave it.
///
/// Leaving does not end either of them. The run waits for whichever it was once the
/// screen is given back, which for a walk is a few more minutes and for a guard is
/// until the location goes or the operator interrupts the process — and what is said
/// on the way out is what says which.
pub(super) fn keeping(
    stage: &mut Stage,
    lasting: &'static Lasting,
    named: String,
    ends: bool,
    mut said: Option<Reading>,
    press: &Press,
) -> Wanted {
    if let Some(reading) = said.as_mut() {
        if moved(reading, press) {
            *stage = Stage::Keeping {
                lasting,
                named,
                ends,
                said,
            };
            return Wanted::Nothing;
        }
    }
    if ends && matches!(*press, Press::Abandon) {
        *stage = Stage::Came(Reading::of(vec![LET_GO.to_owned()]));
        return Wanted::Stop;
    }
    let leaving = super::leaving(press);
    *stage = Stage::Keeping {
        lasting,
        named,
        ends,
        said,
    };
    if leaving {
        return Wanted::Leave;
    }
    Wanted::Nothing
}

/// Put one of a walk's steps on the screen, where a walk is what is running.
///
/// The words are the ones a shell is given for the same step, and so is the rule
/// about which steps are worth saying at all — the choosing line is kept for the
/// report, because at the moment it happens the operator has just typed the thing.
pub(super) fn stepped(stage: &mut Stage, line: &Step) {
    let Stage::Keeping {
        said: Some(reading),
        ..
    } = stage
    else {
        return;
    };
    if is_worth_saying(line.step) {
        reading.put(spoken(line));
    }
}

/// What a walk came to, under the steps that were watched on the way.
///
/// Under them rather than over them, which is what a shell shows: the steps scrolled
/// past and the ending is the next thing printed. A box that threw the walk away and
/// showed only the ending would leave an operator who looked up at the wrong moment
/// with no way back to what they missed.
pub(super) fn came_to(said: Option<Reading>, answer: Vec<String>) -> Stage {
    match said {
        Some(mut reading) => {
            for line in answer {
                reading.put(line);
            }
            Stage::Came(reading)
        }
        None => Stage::Came(Reading::of(answer)),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{all, every, Going, Lasting, KEY, LET_GO};
    use lemonfiber::reaching::ACTS;
    use lemonfiber_api::actions::OFFERED as WEB;
    use lemonfiber_api::jobs::{leased, Lease};
    use lemonfiber_core::app::Command;

    /// One of these naming an action no surface offers, for the path that reports a
    /// translation which reached no command.
    pub(crate) static NOTHING_ANSWERS: Lasting = Lasting {
        name: "a walk nothing answers",
        about: "for the refusal a translation that reaches no command produces",
        action: "not an action any surface offers",
        asks: "Walk through",
        going: Going::Walking("What to look for"),
    };

    /// What one action is called on the list, for the screen's own tests, which move
    /// to an entry by the name an operator would read rather than by number.
    pub(crate) fn listed(action: &str) -> String {
        every()
            .filter(|lasting| lasting.action == action)
            .map(|lasting| lasting.name)
            .collect()
    }

    /// The whole point of naming the action rather than assembling a command here:
    /// what this screen starts has to be something another surface already offers, or
    /// the requirement it is being built for is defeated by the thing built for it.
    #[test]
    fn every_request_this_list_starts_is_one_the_other_surfaces_offer() {
        let missing: Vec<&str> = every()
            .map(|lasting| lasting.action)
            .filter(|action| !WEB.contains(action))
            .collect();

        assert!(missing.is_empty(), "{missing:?}");
    }

    /// The key this list opens on is not one the screen already answers, or the thing
    /// it already did stops happening and nothing says so.
    #[test]
    fn the_key_that_opens_them_is_not_one_the_screen_already_answers() {
        for taken in [
            'q',
            'r',
            '?',
            'y',
            crate::acting::question::KEY,
            crate::acting::errand::KEY,
            crate::acting::surface::KEY,
        ] {
            assert_ne!(KEY, taken, "{taken:?} was already spoken for");
        }
        for offer in crate::acting::offer::OFFERED {
            assert_ne!(KEY, offer.key, "{:?} was already spoken for", offer.key);
        }
    }

    /// Each is named once and says what it does, since the line under the name is the
    /// whole of what somebody chooses between them on.
    #[test]
    fn each_is_named_once_and_says_what_it_does() {
        for lasting in every() {
            let same = every().filter(|other| other.name == lasting.name).count();
            assert_eq!(same, 1, "more than one is called {}", lasting.name);
            assert!(!lasting.about.is_empty(), "{}", lasting.name);
            assert!(!lasting.asks.is_empty(), "{}", lasting.name);
        }
    }

    /// The list opens on the walk and holds them all.
    #[test]
    fn the_list_opens_on_the_walk_and_holds_them_all() {
        let (first, rest) = all();

        assert_eq!(first.action, "walkthrough");
        assert!(matches!(first.going, Going::Walking(_)));
        assert_eq!(rest.len() + 1, every().count());
    }

    /// The one this screen offers to end is the one the web holds on a lease, asked
    /// of the web's own table rather than decided twice. A second list would come to
    /// disagree, and where it disagreed the screen would either offer to end work
    /// that was going to succeed or leave an operator with no way to end work that
    /// never will.
    #[test]
    fn the_only_one_offered_an_end_is_the_one_the_web_leases() {
        let guard = Command::Watch {
            forms: vec!["media".to_owned()],
        };
        let walk = Command::Walkthrough { item: None };

        assert_eq!(leased(&guard), Lease::WhileAsked);
        assert_eq!(leased(&walk), Lease::Held);
        assert_eq!(listed("watch"), "a guard on the data location");
    }

    /// Letting a guard go says that it stopped guarding and stopped nothing else.
    #[test]
    fn letting_a_guard_go_says_the_services_were_left_alone() {
        assert!(LET_GO.contains("as they were"));
    }

    /// The one thing this list is checked against from outside the binary, for the
    /// half of it that names an action. An entry here with no entry there leaves the
    /// parity table's terminal column claiming less than the screen does.
    #[test]
    fn every_action_this_list_names_is_published_for_the_parity_table() {
        let published: Vec<&str> = ACTS.iter().map(|reach| reach.through).collect();

        for lasting in every() {
            assert!(
                published.contains(&lasting.action),
                "{} reaches {} and no entry says so",
                lasting.name,
                lasting.action
            );
        }
    }
}
