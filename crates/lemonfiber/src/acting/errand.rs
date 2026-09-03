//! The rest of what this stack can be told to do, and what becomes of one.
//!
//! Eight errands behind one key. The screen already answers `q`, `r`, `?`, the key
//! that opens the questions and five actions of its own, and there is no letter
//! left that anybody would guess — so this is the arrangement [`super::question`]
//! already made for the reads, made again for the writes that are not the lifecycle
//! five. A ninth errand goes on the list without costing anybody a letter to
//! learn.
//!
//! The key promises only that there is more. A wiring, a capture, a bundle, an
//! archive put back and a revert have no one word between them that is not vaguer
//! than the eight names on the list, and a key claiming something the list does not
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
//! **A bundle is asked what it is to hold.** How much of each service's log to take is
//! typed on a line of its own and what becomes of media filenames is taken off a list,
//! and both are said in the question above the yes — so what is agreed to is what is
//! written. The careful answers are still where an operator who presses enter twice
//! lands: an empty line is the ordinary window, and the list opens on filenames
//! replaced.
//!
//! **Nothing this screen sends can reveal a setting.** Which settings are shown as
//! they are is the one thing about a bundle not offered here, and it is an exception
//! rather than a gap: a way past the withholding list on this surface would be a
//! capability no other surface has, on the surface least likely to be sitting behind a
//! login. The guard beside this list holds it — every bundle this screen can send is
//! held to naming none — which is also what makes the agreement beside it safe to
//! carry.

use lemonfiber_api::actions::{named, Arguments};
use lemonfiber_core::app::bundle::LINES;
use lemonfiber_core::app::{Command, Outcome};

mod given;
mod listed;

use super::bundling;
use super::chooser::{Chooser, Listed};
use super::inviting;
use super::reading::{moved, Reading};
use super::service;
use super::{Press, Stage, Wanted};

pub(crate) use given::{Given, Needs};
pub(super) use listed::all;
#[cfg(test)]
pub(super) use listed::every;

/// How many digits the line a log window is typed on will hold.
///
/// Nine, which is every number the argument can carry and no number it cannot. A line
/// that refuses the tenth digit is a line that can only ever hold an answer, which is
/// why nothing here has to write a sentence about a window that is not a number.
const MOST_DIGITS: usize = 9;

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
    /// How the question before it begins, what it was given completing it.
    pub(crate) asks: &'static str,
    /// What it has to be given first.
    pub(crate) needs: Needs,
    /// The further acceptance this errand's own account can call for, where it can
    /// call for one.
    ///
    /// One errand can. A restore onto a machine whose data root is not the one the
    /// archive was taken against is held until that move is accepted, and the run that
    /// lists what the archive holds is the run that says whether it is. So the words
    /// are here and the fact is the core's, and the question an operator answers is
    /// the one the account in front of it called for.
    accepts: Option<&'static str>,
    /// What it sends once it has been agreed to.
    going: Going,
}

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
            Going::Agreed | Going::Written => Some(self.reaching(given.asked())),
        }
    }

    /// What this errand sends once the operator has agreed to it.
    ///
    /// What it was given, and the careful defaults for everything else a bundle would
    /// otherwise be free to hold.
    ///
    /// The yes is carried as the command's own agreement on both of the two that take
    /// one. It was not, on the bundle: the yes was spent on writing the file and the
    /// field the command reads for consent went out false on every bundle this screen
    /// ever wrote. Nothing about what was produced was different, because this screen
    /// names no setting to show as it is — and that is the point. An agreement that
    /// arrives only when it happens to matter is an agreement nothing carries.
    fn sent(&self, given: &Given) -> Result<Command, String> {
        let mut asked = given.asked();
        match self.going {
            Going::Once => (),
            Going::Agreed => asked.confirm = true,
            Going::Written => {
                asked.write = true;
                asked.confirm = true;
            }
        }
        self.reaching(asked)
    }

    /// Whether what the unconfirmed run reported calls for the further acceptance this
    /// errand can carry, and the words for it where it does.
    ///
    /// The archive's own account of itself says which data root it was taken against,
    /// and a difference there is the one thing a re-point is for. Read off the answer
    /// rather than asked of the operator up front, for the reason the account is put
    /// in front of the question at all: an effect somebody agrees to before hearing of
    /// it is not one they agreed to.
    fn accepting(&self, outcome: &Outcome) -> Option<&'static str> {
        let accepts = self.accepts?;
        match outcome {
            Outcome::Restore(restoration) => {
                restoration.would.relocation.is_some().then_some(accepts)
            }
            _ => None,
        }
    }

    /// The command an errand comes to, or why it comes to none — in the words the
    /// web surface gives for the same request.
    fn reaching(&self, given: Arguments) -> Result<Command, String> {
        named(self.action, given).map_err(|no| no.said())
    }
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
        // Four errands open a line and what they do with the word differs, which is
        // [`given`]'s answer rather than this one's: what is decided here is only
        // that there is a line.
        Needs::Archive(asks)
        | Needs::Bundling(asks)
        | Needs::Named(asks)
        | Needs::Invitation(asks) => {
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
        Press::Accept => return given(stage, errand, typed),
        Press::Rubout => {
            typed.pop();
        }
        Press::Typed(character) => took(errand, &mut typed, character),
        Press::Back | Press::Forward => (),
    }
    *stage = Stage::Naming {
        errand,
        asks,
        typed,
    };
    Wanted::Nothing
}

/// Take the character where this line will have it.
///
/// A log window is a number, so its line takes digits and no more of them than a
/// number the argument can carry. A line that will only ever hold an answer is a line
/// nothing has to write a refusal about: what is turned away is the keystroke, not the
/// request, and every other line takes what it is given.
fn took(errand: &Errand, typed: &mut String, character: char) {
    match errand.needs {
        Needs::Bundling(_) if !character.is_ascii_digit() || typed.len() >= MOST_DIGITS => (),
        _ => typed.push(character),
    }
}

/// What the line an errand was typed on comes to.
///
/// The name of an archive is the whole of what a restore has to be given, so it goes
/// straight to the run that says what would be overwritten. A log window is the first
/// of a bundle's two answers, so it opens the second.
fn given(stage: &mut Stage, errand: &'static Errand, typed: String) -> Wanted {
    match errand.needs {
        // Nothing typed is the ordinary window, which is the one careful default this
        // line can be left at — and the figure is carried rather than left out, so the
        // question above the yes says the number the command was given.
        Needs::Bundling(_) => {
            *stage = bundling::over(errand, typed.parse().unwrap_or(LINES));
            Wanted::Nothing
        }
        // Same line, different argument: which one the word fills is the errand's
        // business rather than the line's.
        Needs::Named(_) => begun(stage, errand, Given::named(typed)),
        // The first of an invitation's three answers, so it opens the second rather
        // than going on. The name is carried rather than folded into the arguments
        // here, because the sentence above the yes says all three together and there
        // is no way to say two of them and add the third.
        Needs::Invitation(_) => {
            *stage = inviting::over(errand, typed);
            Wanted::Nothing
        }
        _ => begun(stage, errand, Given::typed(typed)),
    }
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
///
/// The account is also what decides the question. A restore whose archive names
/// another machine's data root is a re-point, and the yes under that listing is the
/// yes to the move — so the question names it rather than leaving an operator to
/// agree to a restore and be refused for the one thing the listing had just told them.
pub(super) fn weighed(
    errand: &'static Errand,
    given: Given,
    outcome: &Outcome,
    would: Vec<String>,
) -> Stage {
    let given = match errand.accepting(outcome) {
        Some(accepts) => given.repointing(accepts),
        None => given,
    };
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
    use super::{all, every, Errand, Given, Going, Needs, Outcome, Stage, KEY, LINES};
    use crate::acting::offer::OFFERED as KEYED;
    use lemonfiber::reaching::{ACTS, ALSO};
    use lemonfiber_api::actions::{OFFERED as WEB, TAKES_AGREEMENT};
    use lemonfiber_core::app::restore::{Consent, Kept};
    use lemonfiber_core::app::Command;
    use lemonfiber_core::bundle::Filenames;

    /// One errand naming an action no surface offers, for the paths that report a
    /// translation that came to nothing.
    pub(crate) static UNTRANSLATABLE: Errand = Errand {
        name: "an errand nothing answers",
        about: "for the refusal a translation that reaches no command produces",
        action: "not an action any surface offers",
        asks: "Do the impossible",
        needs: Needs::Nothing,
        accepts: None,
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

    /// Inviting somebody opens a line to type their name on, and the word goes on to
    /// what they may watch rather than straight to the question.
    ///
    /// Driven through the stage machinery rather than by building a `Given` by hand,
    /// because the two halves are decided in different places — that there is a line
    /// at all, and what the word opens next — and a test that skipped the first would
    /// pass with no line ever opening. What becomes of the two answers after it is
    /// [`super::super::inviting`]'s, and is held there.
    #[test]
    fn inviting_somebody_opens_a_line_and_the_word_carries_their_name_on() {
        // Every errand, filtered to the one under test: a `let ... else` here would
        // leave a branch nothing reaches, which the gate counts as untested code.
        let errand = every()
            .find(|errand| errand.action == "invite")
            .unwrap_or(all().0);
        let mut stage = Stage::Idle;

        let _ = super::taken(&mut stage, errand, &[]);

        assert!(
            matches!(&stage, Stage::Naming { asks, .. } if asks.contains("Who it is for")),
            "no line was opened to type a name on"
        );

        let _ = super::given(&mut stage, errand, "ana".to_owned());

        assert!(
            matches!(&stage, Stage::Allowing { name, typed, .. }
                if name == "ana" && typed.is_empty()),
            "the name did not carry on to what they may watch"
        );
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

    /// No bundle this screen can send names a setting to show as it is.
    ///
    /// Over every answer the two lines in front of a bundle can be given rather than
    /// over the one nobody typed at, because a guard that only read the default would
    /// pass on a screen that had grown a third line. The agreement beside it is safe
    /// to carry precisely because this holds: an agreement to publish nothing is what
    /// every one of these carries.
    #[test]
    fn no_bundle_this_screen_can_send_names_a_setting_to_reveal() {
        let bundling = [
            (Given::nothing(), LINES, Filenames::Replaced),
            (
                Given::bundled(LINES, Filenames::Replaced),
                LINES,
                Filenames::Replaced,
            ),
            (Given::bundled(1, Filenames::Shown), 1, Filenames::Shown),
            (
                Given::bundled(999_999_999, Filenames::Shown),
                999_999_999,
                Filenames::Shown,
            ),
        ];

        for (given, lines, filenames) in &bundling {
            assert_eq!(
                sending("support").map(|errand| errand.sent(given)),
                Some(Ok(Command::Support {
                    write: true,
                    wanted: lemonfiber_core::app::bundle::Wanted {
                        lines: *lines,
                        filenames: *filenames,
                        reveal: Vec::new(),
                        confirmed: true,
                    },
                    dest: lemonfiber_core::app::support::Destination::Kept,
                }))
            );
        }
    }

    /// A restore that came back as something other than a restoration calls for no
    /// re-point.
    ///
    /// A screen that read one out of the wrong shape would be putting an operator's
    /// agreement to a move on the strength of an answer about something else — and
    /// the shape it would have read it out of is whatever the core answered.
    #[test]
    fn an_answer_that_is_not_a_restoration_calls_for_no_re_point() {
        let restoring = sending("restore");

        let accepting =
            restoring.and_then(|errand| errand.accepting(&Outcome::Version(a_version())));

        assert_eq!(accepting, None);
    }

    /// A version report, which is an answer of a shape no errand ever has.
    fn a_version() -> lemonfiber_core::model::VersionReport {
        lemonfiber_core::model::VersionReport {
            binary: "0.9.0".to_owned(),
            supported_schema: vec![1],
            stack: "1.2.3".to_owned(),
            compose: None,
        }
    }

    /// Asked for with nothing chosen it is asked for with every careful default, and
    /// the yes carries the agreement the command names.
    ///
    /// The agreement is the half that was missing rather than the half that was
    /// wrong: nothing about the file differs, because the reveal is empty either way
    /// — which is exactly why nothing caught it.
    #[test]
    fn the_bundle_this_screen_asks_for_replaces_filenames_and_carries_the_agreement() {
        let bundle = sending("support").map(|errand| errand.sent(&Given::nothing()));

        assert_eq!(
            bundle,
            Some(Ok(Command::Support {
                write: true,
                wanted: lemonfiber_core::app::bundle::Wanted {
                    confirmed: true,
                    ..lemonfiber_core::app::bundle::Wanted::default()
                },
                dest: lemonfiber_core::app::support::Destination::Kept,
            }))
        );
    }

    /// What was chosen in front of the question is what the bundle is asked for.
    ///
    /// The window and the filenames both, because either carried alone would be a
    /// screen that asked two things and sent one.
    #[test]
    fn a_bundle_is_asked_for_with_the_window_and_the_filenames_that_were_chosen() {
        let bundle =
            sending("support").map(|errand| errand.sent(&Given::bundled(20, Filenames::Shown)));

        assert_eq!(
            bundle,
            Some(Ok(Command::Support {
                write: true,
                wanted: lemonfiber_core::app::bundle::Wanted {
                    lines: 20,
                    filenames: Filenames::Shown,
                    reveal: Vec::new(),
                    confirmed: true,
                },
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
