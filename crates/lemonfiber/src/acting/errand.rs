//! The rest of what this stack can be told to do, and what becomes of one.
//!
//! Six errands behind one key. The screen already answers `q`, `r`, `?`, the key
//! that opens the questions and five actions of its own, and there is no letter
//! left that anybody would guess — so this is the arrangement [`super::question`]
//! already made for the reads, made again for the writes that are not the lifecycle
//! five. A seventh errand goes on the list without costing anybody a letter to
//! learn.
//!
//! The key promises only that there is more. A wiring, a capture, a bundle, an
//! archive put back and a revert have no one word between them that is not vaguer
//! than the six names on the list, and a key claiming something the list does not
//! hold is worse than one claiming nothing — so what each errand is is said on the
//! row, where there is room to say it.
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
use super::reading::{moved, Reading};
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

/// One errand this stack can be sent on.
pub(crate) struct Errand {
    /// What it is called on the list, and on the box while it runs.
    pub(crate) name: &'static str,
    /// What it does, in one line.
    pub(crate) about: &'static str,
    /// The name every surface calls this action by.
    pub(crate) action: &'static str,
    /// How the question before it begins, the name it was given completing it.
    pub(crate) asks: &'static str,
    /// What it has to be given first, above the line it is typed on.
    ///
    /// The one errand that takes anything takes the name of a backup, so what is
    /// typed is carried as the archive to restore from.
    pub(crate) names: Option<&'static str>,
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
    names: None,
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
        names: None,
        going: Going::Once,
    },
    Errand {
        name: "a backup",
        about: "capture this configuration to an archive kept on this machine",
        action: "backup",
        asks: "Capture this configuration to an archive",
        names: None,
        going: Going::Once,
    },
    Errand {
        name: "a support bundle",
        about: "what somebody helping would ask for, with every credential replaced",
        action: "support",
        asks: "Write the bundle",
        names: None,
        going: Going::Written,
    },
    Errand {
        name: "a backup put back",
        about: "restore one this machine took, over the configuration here now",
        action: "restore",
        asks: "Restore from",
        names: Some("Which backup, by the name it was written under"),
        going: Going::Agreed,
    },
    Errand {
        name: "your edits thrown away",
        about: "put lemonfiber's own state back over every value you changed",
        action: "reset",
        asks: "Throw away every edit above",
        names: None,
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
    fn would(&self, typed: &str) -> Option<Result<Command, String>> {
        match self.going {
            Going::Once => None,
            Going::Agreed | Going::Written => Some(self.reaching(self.given(typed))),
        }
    }

    /// What this errand sends once the operator has agreed to it.
    fn sent(&self, typed: &str) -> Result<Command, String> {
        let mut given = self.given(typed);
        match self.going {
            Going::Once => (),
            Going::Agreed => given.confirm = true,
            Going::Written => given.write = true,
        }
        self.reaching(given)
    }

    /// What it is given: the name where it takes one, and the careful defaults for
    /// everything else a bundle would otherwise be free to hold.
    ///
    /// Nothing typed is nothing named, rather than an empty name. An empty one is a
    /// name the core would go and fail to find, which costs a round trip to be told
    /// what the translation already knows — the same reading a trace with nothing
    /// typed is given.
    fn given(&self, typed: &str) -> Arguments {
        Arguments {
            archive: self
                .names
                .and_then(|_| (!typed.is_empty()).then(|| typed.to_owned())),
            ..Arguments::default()
        }
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
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return taken(stage, chooser.taken()),
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Sending(chooser);
    Wanted::Nothing
}

/// Send the errand that was taken, or open the line it has to be named on first.
fn taken(stage: &mut Stage, errand: &'static Errand) -> Wanted {
    match errand.names {
        Some(asks) => {
            *stage = Stage::Naming {
                errand,
                asks,
                typed: String::new(),
            };
            Wanted::Nothing
        }
        None => begun(stage, errand, String::new()),
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
        Press::Accept => return begun(stage, errand, typed),
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
fn begun(stage: &mut Stage, errand: &'static Errand, typed: String) -> Wanted {
    match errand.would(&typed) {
        Some(Ok(command)) => {
            *stage = Stage::Weighing { errand, typed };
            Wanted::Carry(command)
        }
        Some(Err(said)) => {
            *stage = Stage::Came(Reading::of(vec![said]));
            Wanted::Nothing
        }
        None => {
            *stage = Stage::Agreeing {
                errand,
                typed,
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
    typed: String,
    press: &Press,
) -> Wanted {
    if matches!(*press, Press::Abandon) {
        return Wanted::Nothing;
    }
    *stage = Stage::Weighing { errand, typed };
    Wanted::Nothing
}

/// What the core said the errand would do, held for the operator to read and answer.
pub(super) fn weighed(errand: &'static Errand, typed: String, would: Vec<String>) -> Stage {
    Stage::Agreeing {
        errand,
        typed,
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
    typed: String,
    mut would: Option<Reading>,
    press: &Press,
) -> Wanted {
    if let Some(reading) = would.as_mut() {
        if moved(reading, press) {
            *stage = Stage::Agreeing {
                errand,
                typed,
                would,
            };
            return Wanted::Nothing;
        }
    }
    if !matches!(*press, Press::Typed('y' | 'Y')) {
        return Wanted::Nothing;
    }
    match errand.sent(&typed) {
        Ok(command) => {
            *stage = Stage::Doing { errand, typed };
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
    typed: String,
    press: &Press,
) -> Wanted {
    *stage = Stage::Doing { errand, typed };
    if matches!(*press, Press::Typed('q') | Press::Abandon) {
        return Wanted::Leave;
    }
    Wanted::Nothing
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{all, every, Errand, Going, KEY};
    use crate::acting::offer::OFFERED as KEYED;
    use lemonfiber::reaching::ACTS;
    use lemonfiber_api::actions::{OFFERED as WEB, TAKES_AGREEMENT};
    use lemonfiber_core::app::restore::Kept;
    use lemonfiber_core::app::Command;

    /// One errand naming an action no surface offers, for the paths that report a
    /// translation that came to nothing.
    pub(crate) static UNTRANSLATABLE: Errand = Errand {
        name: "an errand nothing answers",
        about: "for the refusal a translation that reaches no command produces",
        action: "not an action any surface offers",
        asks: "Do the impossible",
        names: None,
        going: Going::Once,
    };

    /// The errand one action is on, for a test that wants a particular one.
    fn sending(action: &str) -> Option<&'static Errand> {
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

    /// The one thing this list is checked against from outside the binary. An
    /// action offered here with no entry there leaves the parity table's terminal
    /// column claiming less than the screen does, and an entry there naming an
    /// action nothing offers leaves it claiming more.
    #[test]
    fn every_action_this_screen_offers_is_published_for_the_parity_table() {
        let mut offered: Vec<&str> = KEYED
            .iter()
            .map(|offer| offer.action)
            .chain(every().map(|errand| errand.action))
            .collect();
        let mut published: Vec<&str> = ACTS.iter().map(|reach| reach.through).collect();
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
            let says = errand.would("a-backup.tar.gz").is_some();
            assert_eq!(says, takes, "{}", errand.name);
        }
    }

    /// A bundle asked for here is asked for with every careful default: the
    /// ordinary window of log lines, filenames replaced, and nothing revealed. A
    /// terminal-only way past any of those would be a surface showing what no other
    /// surface shows.
    #[test]
    fn the_bundle_this_screen_asks_for_reveals_nothing_and_replaces_filenames() {
        let bundle = sending("support").map(|errand| errand.sent(""));

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
        let would = sending("support").and_then(|errand| errand.would(""));

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

        let climbing = restoring.map(|errand| errand.sent("../../etc/shadow"));
        assert_eq!(
            climbing,
            Some(Ok(Command::Restore {
                archive: Kept::Named("../../etc/shadow".to_owned()),
                repoint: false,
                confirm: true,
            }))
        );
        let listing = restoring.and_then(|errand| errand.would("lemonfiber-full-1.tar.gz"));
        assert!(matches!(
            listing,
            Some(Ok(Command::Restore { confirm: false, .. }))
        ));
    }

    /// A restore with nothing typed is refused in the sentence the web surface
    /// gives for the same request, rather than in one this screen wrote.
    #[test]
    fn a_restore_with_no_name_is_refused_in_the_words_the_other_surface_gives() {
        let said = sending("restore")
            .and_then(|errand| errand.would("").and_then(Result::err))
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
            errand.and_then(|errand| errand.would("")),
            Some(Ok(Command::Reset { confirm: false }))
        );
        assert_eq!(
            errand.map(|errand| errand.sent("")),
            Some(Ok(Command::Reset { confirm: true }))
        );
    }

    /// The three that carry no agreement are sent as they stand, and each reaches
    /// the command every other surface produces for the same request.
    #[test]
    fn an_errand_carrying_no_agreement_is_sent_as_it_stands() {
        let sent: Vec<Result<Command, String>> = ["seed", "adopt", "backup"]
            .into_iter()
            .filter_map(sending)
            .map(|errand| errand.sent(""))
            .collect();

        assert_eq!(
            sent,
            vec![
                Ok(Command::Seed),
                Ok(Command::Adopt),
                Ok(Command::Backup { service: None }),
            ]
        );
    }

    /// The list opens on the first errand and holds every one of them.
    #[test]
    fn the_list_opens_on_the_first_errand_and_holds_them_all() {
        let (first, rest) = all();

        assert_eq!(first.action, "seed");
        assert_eq!(rest.len() + 1, every().count());
        assert!(first.names.is_none());
    }
}
