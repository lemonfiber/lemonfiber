//! The rest of what this stack can be told to do, and what becomes of one.
//!
//! Seven errands behind one key. The screen already answers `q`, `r`, `?`, the key
//! that opens the questions and five actions of its own, and there is no letter
//! left that anybody would guess — so this is the arrangement [`super::question`]
//! already made for the reads, made again for the writes that are not the lifecycle
//! five. An eighth errand goes on the list without costing anybody a letter to
//! learn.
//!
//! The key promises only that there is more. A wiring, a capture, a bundle, an
//! archive put back and a revert have no one word between them that is not vaguer
//! than the seven names on the list, and a key claiming something the list does not
//! hold is worse than one claiming nothing — so what each errand is is said on the
//! row, where there is room to say it.
//!
//! **Putting the last repair back is one of these, and putting a fault right is
//! not.** The two are one errand read in both directions, and the command line says
//! so — `--fix` and `--undo` are two errands rather than four settings, which is why
//! it declares them apart. But only one of them fits this list's rule. A repair is
//! offered, read, and then agreed to *in part*: the yes is a selection out of an
//! offer, which is the one thing no errand here does. An undo reads nothing and names
//! nothing — which repair was last, what reversing it takes and which of those need a
//! service to reach are the core's to decide — so its yes is the whole of the
//! agreement, exactly as the wiring's and the capture's are. It sits beside the other
//! reversal, the narrower of the two first, so nobody reaching for the one that puts
//! back a single repair lands on the one that puts back the whole configuration.
//!
//! Each errand is held by the name every surface calls the action by, and that name
//! goes through [`lemonfiber_api::actions::named`] exactly as the five on their own
//! keys do. What an errand may be given is that table's too: the arguments are the
//! ones the command carries, and one it has nowhere to put is refused there rather
//! than dropped here.
//!
//! **What it would do is said before the question, not after.** Three of the six
//! answer what they would come to without changing anything — the reverts a reset
//! would make, what a bundle would hold, what an archive would overwrite — so those
//! three are asked that first, and the question goes under the answer. An effect
//! somebody learns about afterwards is not something they agreed to, which is the
//! reading `doctor --fix` already puts its own offers under.
//!
//! **A name, never a path.** The one errand that has to be given something is the
//! restore, and what it takes is the name a backup was written under. It goes
//! through the same translation a browser's does, which carries it as
//! [`lemonfiber_core::app::restore::Kept::Named`] and never as a path — so the name
//! is resolved beneath the backups directory by the core, and one holding a path or
//! climbing out of that directory is refused by name rather than followed.
//!
//! **Nothing this screen sends can reveal a setting.** A bundle asked for here is
//! asked for with the careful defaults: the ordinary window of log lines, media
//! filenames replaced, and nothing revealed. What a bundle withholds is decided
//! where the bundle is made, and a terminal-only way past it would be a surface
//! showing a credential no other surface shows.

use lemonfiber_api::actions::{named, Arguments};
use lemonfiber_core::app::Command;

use super::chooser::{Chooser, Listed};
use super::offer::Taken;
use super::reading::{moved, Reading};
use super::service;
use super::{Press, Stage, Wanted};

/// The key that opens the rest of the errands.
pub(crate) const KEY: char = 'm';

/// The word the footer puts beside that key.
pub(crate) const HINT: &str = "more";

/// What an errand sends once it has been agreed to.
enum Going {
    /// As it stands. The question is the whole of the agreement, because the
    /// command has no half that reports and changes nothing.
    Once,
    /// The agreement itself, the run before it having said what would be lost.
    Agreed,
    /// The file, the run before it having said what would go in it.
    Written,
}

/// What an errand has to be given before it can be sent.
///
/// Two of the six take something, and which of the two shapes each takes is decided
/// the way the questions on the other list decide it: by where the thing being named
/// already is. An archive is written under a name nothing on this screen is holding,
/// so it is typed. A service is on the panel this box is drawn over, so it is taken
/// off a list — and a typed service name would be a name nothing checked before the
/// capture ran.
pub(crate) enum Needs {
    /// Nothing: it is sent as it stands.
    Nothing,
    /// The name a backup was written under, typed on a line of its own, with what is
    /// asked for above it.
    Archive(&'static str),
    /// One of the services the screen has in hand, or the whole stack.
    Service,
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
    pub(super) fn nothing() -> Self {
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
    pub(super) fn typed(typed: String) -> Self {
        Self {
            asked: Arguments {
                archive: (!typed.is_empty()).then(|| typed.clone()),
                ..Arguments::default()
            },
            said: typed,
        }
    }

    /// The whole of what the errand is about, where there was nothing to narrow it to.
    ///
    /// Said rather than left out. A capture with no service to choose between is the
    /// whole stack, and the question it is put reads as a sentence with its subject
    /// missing if nothing says so — which is exactly the screen an operator gets when
    /// the container engine could not be reached.
    pub(super) fn whole(said: &str) -> Self {
        Self {
            asked: Arguments::default(),
            said: said.to_owned(),
        }
    }

    /// The service that was taken off the list, or the whole stack where the row
    /// naming none was.
    pub(super) fn picked(taken: &Taken) -> Self {
        Self {
            asked: Arguments {
                service: taken.named().into_iter().next(),
                ..Arguments::default()
            },
            said: taken.name(),
        }
    }

    /// What the question calls it.
    pub(super) fn said(&self) -> &str {
        &self.said
    }
}

/// One errand this stack can be sent on.
pub(crate) struct Errand {
    /// What it is called on the list, and on the box while it runs.
    pub(crate) name: &'static str,
    /// What it does, in one line.
    pub(crate) about: &'static str,
    /// The name every surface calls this action by.
    pub(crate) action: &'static str,
    /// How the question before it begins, what it was given completing it.
    pub(crate) asks: &'static str,
    /// What it has to be given first.
    pub(crate) needs: Needs,
    /// What it sends once it has been agreed to.
    going: Going,
}

/// The errand the list opens on.
///
/// Held apart from the rest for the reason the selected question is: a list built
/// from a slice that might have been empty carries a case for there being no
/// errands, which is not a state this screen can be in.
static OPENS_ON: Errand = Errand {
    name: "wiring",
    about: "connect each service to the others, leaving what you changed alone",
    action: "seed",
    asks: "Wire the services to each other",
    needs: Needs::Nothing,
    going: Going::Once,
};

/// The errands after it, read from the ones that keep work towards the one that
/// throws it away — which is also the order nobody lands on the destructive one by
/// pressing enter at the list.
static AFTER: &[Errand] = &[
    Errand {
        name: "your edits kept",
        about: "take every value you changed as lemonfiber's own, so a seed leaves it",
        action: "adopt",
        asks: "Keep every value you changed",
        needs: Needs::Nothing,
        going: Going::Once,
    },
    Errand {
        name: "a backup",
        about: "capture a configuration to an archive kept on this machine",
        action: "backup",
        asks: "Capture the configuration of",
        needs: Needs::Service,
        going: Going::Once,
    },
    Errand {
        name: "a support bundle",
        about: "what somebody helping would ask for, with every credential replaced",
        action: "support",
        asks: "Write the bundle",
        needs: Needs::Nothing,
        going: Going::Written,
    },
    Errand {
        name: "the last repair put back",
        about: "reverse what the last repair changed, leaving the wiring under it alone",
        action: "undo",
        asks: "Put back what the last repair changed",
        needs: Needs::Nothing,
        going: Going::Once,
    },
    Errand {
        name: "a backup put back",
        about: "restore one this machine took, over the configuration here now",
        action: "restore",
        asks: "Restore from",
        needs: Needs::Archive("Which backup, by the name it was written under"),
        going: Going::Agreed,
    },
    Errand {
        name: "your edits thrown away",
        about: "put lemonfiber's own state back over every value you changed",
        action: "reset",
        asks: "Throw away every edit above",
        needs: Needs::Nothing,
        going: Going::Agreed,
    },
];

impl Listed for Errand {
    fn name(&self) -> &str {
        self.name
    }

    fn about(&self) -> &str {
        self.about
    }
}

impl Errand {
    /// What this errand would do, or nothing where it has no half that only reports.
    ///
    /// The arguments as they stand: unconfirmed, and writing nothing. Those are the
    /// runs the command answers with what it would come to, having touched nothing.
    fn would(&self, given: &Given) -> Option<Result<Command, String>> {
        match self.going {
            Going::Once => None,
            Going::Agreed | Going::Written => Some(self.reaching(given.asked.clone())),
        }
    }

    /// What this errand sends once the operator has agreed to it.
    ///
    /// What it was given, and the careful defaults for everything else a bundle would
    /// otherwise be free to hold.
    fn sent(&self, given: &Given) -> Result<Command, String> {
        let mut asked = given.asked.clone();
        match self.going {
            Going::Once => (),
            Going::Agreed => asked.confirm = true,
            Going::Written => asked.write = true,
        }
        self.reaching(asked)
    }

    /// The command an errand comes to, or why it comes to none — in the words the
    /// web surface gives for the same request.
    fn reaching(&self, given: Arguments) -> Result<Command, String> {
        named(self.action, given).map_err(|no| no.said())
    }
}

/// The errands, the one the list opens on apart from the rest.
pub(super) fn all() -> (&'static Errand, Vec<&'static Errand>) {
    (&OPENS_ON, AFTER.iter().collect())
}

/// Every errand, in the order they are read.
#[cfg(test)]
pub(super) fn every() -> impl Iterator<Item = &'static Errand> {
    std::iter::once(&OPENS_ON).chain(AFTER)
}

/// Over the errands: move, take one, or leave it.
pub(super) fn sending(
    stage: &mut Stage,
    mut chooser: Chooser<&'static Errand>,
    press: &Press,
    services: &[(String, String, String)],
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return taken(stage, chooser.taken(), services),
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Sending(chooser);
    Wanted::Nothing
}

/// Send the errand that was taken, or open what it has to be given first.
///
/// A capture with no services to choose between is sent as it stands, which is the
/// whole stack — the request this screen has always made of it. A screen that could
/// not reach the engine has nothing to narrow to, and a list of one row would be
/// offering a choice that is not one.
fn taken(
    stage: &mut Stage,
    errand: &'static Errand,
    services: &[(String, String, String)],
) -> Wanted {
    match errand.needs {
        Needs::Archive(asks) => {
            *stage = Stage::Naming {
                errand,
                asks,
                typed: String::new(),
            };
            Wanted::Nothing
        }
        Needs::Service => match service::for_the_errand(errand, services) {
            Some(inside) => {
                *stage = inside;
                Wanted::Nothing
            }
            None => begun(stage, errand, service::nothing_to_choose()),
        },
        Needs::Nothing => begun(stage, errand, Given::nothing()),
    }
}

/// Over the line being typed: type, take back, go on, or leave it.
pub(super) fn naming(
    stage: &mut Stage,
    errand: &'static Errand,
    asks: &'static str,
    mut typed: String,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return begun(stage, errand, Given::typed(typed)),
        Press::Rubout => {
            typed.pop();
        }
        Press::Typed(character) => typed.push(character),
        Press::Back | Press::Forward => (),
    }
    *stage = Stage::Naming {
        errand,
        asks,
        typed,
    };
    Wanted::Nothing
}

/// Ask what the errand would do, or put the question where it has nothing to say
/// first.
pub(super) fn begun(stage: &mut Stage, errand: &'static Errand, given: Given) -> Wanted {
    match errand.would(&given) {
        Some(Ok(command)) => {
            *stage = Stage::Weighing { errand, given };
            Wanted::Carry(command)
        }
        Some(Err(said)) => {
            *stage = Stage::Came(Reading::of(vec![said]));
            Wanted::Nothing
        }
        None => {
            *stage = Stage::Agreeing {
                errand,
                given,
                would: None,
            };
            Wanted::Nothing
        }
    }
}

/// While what it would do is with the core: back out, or wait for it.
pub(super) fn weighing(
    stage: &mut Stage,
    errand: &'static Errand,
    given: Given,
    press: &Press,
) -> Wanted {
    if matches!(*press, Press::Abandon) {
        return Wanted::Nothing;
    }
    *stage = Stage::Weighing { errand, given };
    Wanted::Nothing
}

/// What the core said the errand would do, held for the operator to read and answer.
pub(super) fn weighed(errand: &'static Errand, given: Given, would: Vec<String>) -> Stage {
    Stage::Agreeing {
        errand,
        given,
        would: Some(Reading::of(would)),
    }
}

/// At the question: move through what it would do, agree to it, or leave it.
///
/// Only an explicit yes goes ahead, the way the teardown's own question is read and
/// the way each repair is offered. Everything else that is not a move puts the box
/// away and changes nothing.
pub(super) fn agreeing(
    stage: &mut Stage,
    errand: &'static Errand,
    given: Given,
    mut would: Option<Reading>,
    press: &Press,
) -> Wanted {
    if let Some(reading) = would.as_mut() {
        if moved(reading, press) {
            *stage = Stage::Agreeing {
                errand,
                given,
                would,
            };
            return Wanted::Nothing;
        }
    }
    if !matches!(*press, Press::Typed('y' | 'Y')) {
        return Wanted::Nothing;
    }
    match errand.sent(&given) {
        Ok(command) => {
            *stage = Stage::Doing { errand, given };
            Wanted::Carry(command)
        }
        Err(said) => {
            *stage = Stage::Came(Reading::of(vec![said]));
            Wanted::Nothing
        }
    }
}

/// While the errand is with the core: leaving is the only thing left to ask.
pub(super) fn doing(
    stage: &mut Stage,
    errand: &'static Errand,
    given: Given,
    press: &Press,
) -> Wanted {
    *stage = Stage::Doing { errand, given };
    if super::leaving(press) {
        return Wanted::Leave;
    }
    Wanted::Nothing
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{all, every, Errand, Given, Going, Needs, KEY};
    use crate::acting::offer::OFFERED as KEYED;
    use lemonfiber::reaching::{ACTS, ALSO};
    use lemonfiber_api::actions::{OFFERED as WEB, TAKES_AGREEMENT};
    use lemonfiber_core::app::restore::{Consent, Kept};
    use lemonfiber_core::app::Command;

    /// One errand naming an action no surface offers, for the paths that report a
    /// translation that came to nothing.
    pub(crate) static UNTRANSLATABLE: Errand = Errand {
        name: "an errand nothing answers",
        about: "for the refusal a translation that reaches no command produces",
        action: "not an action any surface offers",
        asks: "Do the impossible",
        needs: Needs::Nothing,
        going: Going::Once,
    };

    /// An errand given a name typed on a line, which is how the two that take one
    /// were given theirs before either of them existed.
    fn typed(name: &str) -> Given {
        Given::typed(name.to_owned())
    }

    /// The errand one action is on, for a test that wants a particular one.
    pub(crate) fn sending(action: &str) -> Option<&'static Errand> {
        every().find(|errand| errand.action == action)
    }

    /// What one action is called on the list, for the screen's own tests, which
    /// move to an errand by the name an operator would read rather than by number.
    pub(crate) fn listed(action: &str) -> String {
        every()
            .filter(|errand| errand.action == action)
            .map(|errand| errand.name)
            .collect()
    }

    /// The whole point of naming the action rather than assembling a command here:
    /// what this screen sends has to be something another surface already offers,
    /// or the requirement it is being built for is defeated by the thing built for
    /// it.
    #[test]
    fn every_errand_this_screen_sends_is_one_the_other_surfaces_offer() {
        let missing: Vec<&str> = every()
            .map(|errand| errand.action)
            .filter(|action| !WEB.contains(action))
            .collect();

        assert!(missing.is_empty(), "{missing:?}");
    }

    /// An errand is named once, or the second is unreachable on a list that shows
    /// both and nobody would know which they took.
    #[test]
    fn no_two_errands_go_by_the_same_name() {
        for errand in every() {
            let same = every().filter(|other| other.name == errand.name).count();
            assert_eq!(same, 1, "more than one errand is called {}", errand.name);
        }
        assert!(every().all(|errand| !errand.about.is_empty()));
    }

    /// The key this list opens on is not one the screen already answers, or the
    /// thing it already did stops happening and nothing says so.
    #[test]
    fn the_key_that_opens_them_is_not_one_the_screen_already_answers() {
        for taken in ['q', 'r', '?', crate::acting::question::KEY] {
            assert_ne!(KEY, taken, "{taken:?} was already spoken for");
        }
        assert!(KEYED.iter().all(|offer| offer.key != KEY));
    }

    /// The one thing the screen's actions are checked against from outside the
    /// binary, and the whole of what it offers rather than this file's share of it —
    /// the projection is one list, so only one place can hold it to be exactly what
    /// the screen offers. An action offered anywhere with no entry there leaves the
    /// parity table's terminal column claiming less than the screen does, and an
    /// entry there naming an action nothing offers leaves it claiming more.
    ///
    /// Six places offer one: the keys, this list, the two that keep going, the three
    /// quality changes, the two that answer a diagnosis, and the widening offered
    /// under one, which is on no list at all because there is one of it.
    ///
    /// The published names come off two lists because the requests do. Every other
    /// action is the only way its request is reached; the three quality writes and
    /// the widened diagnosis act on requests the screen already reaches as reads, so
    /// they are published beside the rest rather than among them — and both lists are
    /// read here, or a write could be added to the screen and excused by the list it
    /// was not on.
    #[test]
    fn every_action_this_screen_offers_is_published_for_the_parity_table() {
        let mut offered: Vec<&str> = KEYED
            .iter()
            .map(|offer| offer.action)
            .chain(every().map(|errand| errand.action))
            .chain(crate::acting::lasting::every().map(|lasting| lasting.action))
            .chain(crate::acting::quality::every().map(|change| change.action))
            .chain(crate::acting::mending::every().map(|mending| mending.action))
            .chain(std::iter::once(crate::acting::disturbing::ACTION))
            .collect();
        let mut published: Vec<&str> = ACTS.iter().chain(ALSO).map(|reach| reach.through).collect();
        offered.sort_unstable();
        published.sort_unstable();

        assert_eq!(offered, published);
    }

    /// Which errands say what they would do first is asked of the table that says
    /// which actions carry an agreement, rather than being decided again here — a
    /// second list would come to disagree with the first, and the place it would
    /// disagree is in front of somebody about to throw work away.
    #[test]
    fn an_errand_whose_action_takes_an_agreement_says_what_it_would_do_first() {
        for errand in every() {
            let takes = TAKES_AGREEMENT.contains(&errand.action);
            let says = errand.would(&typed("a-backup.tar.gz")).is_some();
            assert_eq!(says, takes, "{}", errand.name);
        }
    }

    /// A bundle asked for here is asked for with every careful default: the
    /// ordinary window of log lines, filenames replaced, and nothing revealed. A
    /// terminal-only way past any of those would be a surface showing what no other
    /// surface shows.
    #[test]
    fn the_bundle_this_screen_asks_for_reveals_nothing_and_replaces_filenames() {
        let bundle = sending("support").map(|errand| errand.sent(&Given::nothing()));

        assert_eq!(
            bundle,
            Some(Ok(Command::Support {
                write: true,
                wanted: lemonfiber_core::app::bundle::Wanted::default(),
                dest: lemonfiber_core::app::support::Destination::Kept,
            }))
        );
    }

    /// What a bundle would hold is asked for before it is written, and asking costs
    /// nothing: the run that says is the run that produces no file.
    #[test]
    fn what_a_bundle_would_hold_is_asked_for_before_one_is_written() {
        let would = sending("support").and_then(|errand| errand.would(&Given::nothing()));

        assert!(matches!(
            would,
            Some(Ok(Command::Support { write: false, .. }))
        ));
    }

    /// A restore is asked for by the name a backup was written under, carried as a
    /// name and never as a path — so a name holding one reaches the core's own
    /// refusal rather than the file it points at.
    #[test]
    fn a_restore_carries_the_name_it_was_given_and_never_a_path() {
        let restoring = sending("restore");

        let climbing = restoring.map(|errand| errand.sent(&typed("../../etc/shadow")));
        assert_eq!(
            climbing,
            Some(Ok(Command::Restore {
                archive: Kept::Named("../../etc/shadow".to_owned()),
                repoint: false,
                consent: Consent::Standing,
            }))
        );
        let listing = restoring.and_then(|errand| errand.would(&typed("lemonfiber-full-1.tar.gz")));
        assert!(matches!(
            listing,
            Some(Ok(Command::Restore {
                consent: Consent::List,
                ..
            }))
        ));
    }

    /// A restore with nothing typed is refused in the sentence the web surface
    /// gives for the same request, rather than in one this screen wrote.
    #[test]
    fn a_restore_with_no_name_is_refused_in_the_words_the_other_surface_gives() {
        let said = sending("restore")
            .and_then(|errand| errand.would(&typed("")).and_then(Result::err))
            .unwrap_or_default();

        assert!(said.contains("restore"), "{said}");
        assert!(said.contains("archive"), "{said}");
    }

    /// A reset is the agreement and nothing else: the run before it reverts nothing
    /// and only names what would be lost.
    #[test]
    fn a_reset_reverts_nothing_until_it_is_agreed_to() {
        let errand = sending("reset");

        assert_eq!(
            errand.and_then(|errand| errand.would(&Given::nothing())),
            Some(Ok(Command::Reset { confirm: false }))
        );
        assert_eq!(
            errand.map(|errand| errand.sent(&Given::nothing())),
            Some(Ok(Command::Reset { confirm: true }))
        );
    }

    /// The four that carry no agreement are sent as they stand, and each reaches
    /// the command every other surface produces for the same request.
    ///
    /// Putting the last repair back is one of them, and it carries no subject either:
    /// which repair was last and what reversing it takes are the core's, so there is
    /// nothing here for this screen to name.
    #[test]
    fn an_errand_carrying_no_agreement_is_sent_as_it_stands() {
        let sent: Vec<Result<Command, String>> = ["seed", "adopt", "backup", "undo"]
            .into_iter()
            .filter_map(sending)
            .map(|errand| errand.sent(&Given::nothing()))
            .collect();

        assert_eq!(
            sent,
            vec![
                Ok(Command::Seed),
                Ok(Command::Adopt),
                Ok(Command::Backup { service: None }),
                Ok(Command::Undo),
            ]
        );
    }

    /// The list opens on the first errand and holds every one of them.
    #[test]
    fn the_list_opens_on_the_first_errand_and_holds_them_all() {
        let (first, rest) = all();

        assert_eq!(first.action, "seed");
        assert_eq!(rest.len() + 1, every().count());
        assert!(matches!(first.needs, Needs::Nothing));
    }
}
