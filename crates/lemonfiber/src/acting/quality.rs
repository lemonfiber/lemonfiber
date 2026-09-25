//! How good this library is to be, and the two ways a choice is put into force.
//!
//! Three writes behind one key, beside the reading they are all about. `quality` is
//! already one of the questions [`super::question`] opens — the preset in force,
//! what each one means and what it costs — and these three are the whole of what can
//! be done about what that reading reports. They are on a key of their own rather
//! than on the list of errands, for a reason that is not tidiness: the agreement does
//! not mean the same thing there.
//!
//! **An errand's unconfirmed run is a rehearsal. A quality choice's is the choice.**
//! Every errand that carries an agreement answers, unconfirmed, with what it would do
//! and changes nothing — which is what makes that run the account its question sits
//! under. `quality-set` is not built that way. Unconfirmed it *records* the choice,
//! and holds it only where this host would have to transcode the result in software,
//! which is the one cost its agreement is for. A list whose rule is "unconfirmed says
//! what it would do" cannot take an action for which that is false without the rule
//! quietly becoming untrue for the ones it was written for — and the place it would
//! become untrue is in front of somebody about to throw work away.
//!
//! **So what goes in front of each question is what that change actually has to
//! say**, and the three do not have the same thing to say:
//!
//! - Choosing is made off the four presets, each with what it means and roughly what
//!   an hour of it costs. The account is the list the choice comes off, because the
//!   run that would otherwise have stated it is the run that records it.
//! - Re-asserting has nothing to say first. It carries no agreement, and the core's
//!   report-only half of it is behind `--dry-run`, which is a property of a run
//!   rather than of a request — so no surface's action can ask for one. A preamble
//!   invented here for symmetry would be this screen claiming a rehearsal happened.
//! - Upgrading says what it would cost before anything is fetched: each media type,
//!   the bar in force for it, and roughly what an hour of that takes. That run
//!   triggers nothing, which is the errands' own pattern arriving where it fits.
//!
//! **The agreement is carried once the account has been read, and never before.**
//! Which of the three carries one at all is asked of
//! [`lemonfiber_api::actions::TAKES_AGREEMENT`] rather than written down again here:
//! a second list would come to disagree with the first, and it would disagree in
//! front of somebody about to spend a week of their connection.
//!
//! **A cost this host would pay is read before it is agreed to.** A choice this
//! machine could only transcode in software comes back held rather than recorded,
//! and the caution the core answers with is the account the second question sits
//! under. It arrives there rather than before the first question because whether
//! there is a cost at all depends on the media server and the platform, which are
//! the core's to know and not this screen's to guess.

mod chosen;

use lemonfiber_api::actions::{named, TAKES_AGREEMENT};
use lemonfiber_core::app::{Command, Outcome};
use lemonfiber_core::model::Disposition;
use lemonfiber_core::recyclarr::Kind;

use super::chooser::{Chooser, Listed};
use super::reading::{moved, Reading};
use super::{Press, Stage, Wanted};

pub(crate) use chosen::{Chosen, Grade, Scope};

/// The key that opens the three.
///
/// Not the letter the word begins with: `q` closes the screen, and a key that quit
/// for eleven slices and then changed its mind would be worse than an arbitrary one.
pub(crate) const KEY: char = 'c';

/// The word the footer puts beside that key.
pub(crate) const HINT: &str = "quality";

/// What is put in front of the question.
enum Before {
    /// The four presets, each with what it means and what it costs. The choice is
    /// made off the account rather than after it, because the run that would
    /// otherwise state the consequence is the run that records the choice.
    Presets,
    /// Nothing. The action carries no agreement and has no half that only reports,
    /// so there is nothing to put there that would be true.
    Nothing,
    /// The action's own unconfirmed run, which states what it would cost and
    /// triggers nothing.
    Cost,
}

/// One change to the quality this stack aims for.
pub(crate) struct Change {
    /// What it is called on the list, and on the box while it runs.
    pub(crate) name: &'static str,
    /// What it does, in one line.
    pub(crate) about: &'static str,
    /// The name every surface calls this action by.
    pub(crate) action: &'static str,
    /// How the question before it begins, what it was chosen completing it.
    pub(crate) asks: &'static str,
    /// What is put in front of that question.
    before: Before,
}

/// The change the list opens on.
///
/// Held apart from the rest for the reason the selected errand and the selected
/// question are: a list built from a slice that might have been empty carries a case
/// for there being nothing to choose, which is not a state this screen can be in.
static OPENS_ON: Change = Change {
    name: "how good it should be",
    about: "choose what future downloads aim for; nothing already downloaded changes",
    action: "quality-set",
    asks: "Aim for",
    before: Before::Presets,
};

/// The two after it, read from the one that changes what happens next towards the
/// one that spends the connection — which is also the order nobody starts a
/// week-long download by pressing enter at a list they have only just opened.
static AFTER: &[Change] = &[
    Change {
        name: "the choice put back over your edits",
        about: "let the chosen preset overwrite the quality file you edited by hand",
        action: "quality-reapply",
        asks: "Overwrite your own edits with the chosen preset",
        before: Before::Nothing,
    },
    Change {
        name: "what is already here, upgraded",
        about: "fetch the library again at the quality chosen, which costs bandwidth and days",
        action: "quality-upgrade",
        asks: "Fetch everything again at the quality chosen",
        before: Before::Cost,
    },
];

impl Listed for Change {
    fn name(&self) -> &str {
        self.name
    }

    fn about(&self) -> &str {
        self.about
    }
}

impl Change {
    /// What this change sends, given what it was chosen and whether the account in
    /// front of the question has been read.
    ///
    /// The agreement goes on once there is an account and never before it. Whether
    /// this action carries one at all is the web's own table's answer rather than a
    /// second one kept here, so an action that stopped taking an agreement stops
    /// being sent one on this screen too.
    fn sent(&self, chosen: &Chosen, read: bool) -> Result<Command, String> {
        let mut given = chosen.asked();
        given.confirm = read && TAKES_AGREEMENT.contains(&self.action);
        named(self.action, given).map_err(|no| no.said())
    }

    /// What a choice can be made about, or the refusal where it can be made about
    /// nothing.
    ///
    /// The whole library, then each kind of media the quality model configures, then
    /// music. Every one of them goes through the translation carrying every bar in
    /// turn, and a scope no bar at all reaches a command for is not offered — which is
    /// the rule the five actions on their own keys build their subjects by, and the
    /// reason no list of media types is written down on this screen.
    fn scopes(&self) -> Result<(Scope, Vec<Scope>), String> {
        let offered = std::iter::once(Scope::everything())
            .chain(Kind::ALL.into_iter().map(Scope::kind))
            .chain(std::iter::once(Scope::music()));
        let mut scopes = Vec::new();
        let mut refused = String::new();
        for scope in offered {
            match self.grades(&Chosen::media(&scope)) {
                Ok(_) => scopes.push(scope),
                Err(said) => refused = said,
            }
        }
        let mut offered = scopes.into_iter();
        match offered.next() {
            Some(first) => Ok((first, offered.collect())),
            None => Err(refused),
        }
    }

    /// The bars this change can be given for that media, or the refusal where it can
    /// be given none.
    ///
    /// Every bar goes through the translation and only what comes to a command is
    /// offered. That is what makes music three audio formats where everything else is
    /// four resolution presets: the same list is put to the same table, and the table
    /// keeps what the action it reaches can carry. The first comes back apart from the
    /// rest, so what is handed on is a list something is already selected in.
    fn grades(&self, about: &Chosen) -> Result<(Grade, Vec<Grade>), String> {
        let mut grades = Vec::new();
        let mut refused = String::new();
        for grade in Grade::every() {
            match self.sent(&about.graded(grade.name), false) {
                Ok(_) => grades.push(grade),
                Err(said) => refused = said,
            }
        }
        let mut offered = grades.into_iter();
        match offered.next() {
            Some(first) => Ok((first, offered.collect())),
            None => Err(refused),
        }
    }
}

/// The three, the one the list opens on apart from the rest.
pub(super) fn all() -> (&'static Change, Vec<&'static Change>) {
    (&OPENS_ON, AFTER.iter().collect())
}

/// Every one of them, in the order they are read.
#[cfg(test)]
pub(super) fn every() -> impl Iterator<Item = &'static Change> {
    std::iter::once(&OPENS_ON).chain(AFTER)
}

/// Over the list: move, take one, or leave it.
pub(super) fn deciding(
    stage: &mut Stage,
    mut chooser: Chooser<&'static Change>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => return taken(stage, chooser.taken()),
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Deciding(chooser);
    Wanted::Nothing
}

/// Take the one selected: open the media a choice is about, ask what it would cost,
/// or put the question where the change has nothing to say first.
fn taken(stage: &mut Stage, change: &'static Change) -> Wanted {
    match change.before {
        Before::Presets => match change.scopes() {
            Ok((first, rest)) => {
                *stage = Stage::Scoping {
                    change,
                    chooser: Chooser::over(first, rest),
                };
                Wanted::Nothing
            }
            Err(said) => came(stage, said),
        },
        Before::Nothing => {
            *stage = Stage::Settling {
                change,
                chosen: Chosen::nothing(),
                account: None,
            };
            Wanted::Nothing
        }
        Before::Cost => match change.sent(&Chosen::nothing(), false) {
            Ok(command) => {
                *stage = Stage::Costing { change };
                Wanted::Carry(command)
            }
            Err(said) => came(stage, said),
        },
    }
}

/// Over the media a choice can be about: move, take one, or leave it.
///
/// Taking one opens the bars that media can be given, which is the same list put to
/// the same table with a different media named — so music comes back as three audio
/// formats and everything else as four resolution presets, without this screen
/// knowing which is which.
pub(super) fn scoping(
    stage: &mut Stage,
    change: &'static Change,
    mut chooser: Chooser<Scope>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => {
            let chosen = Chosen::media(&chooser.taken());
            return match change.grades(&chosen) {
                Ok((first, rest)) => {
                    *stage = Stage::Grading {
                        change,
                        chosen,
                        chooser: Chooser::over(first, rest),
                    };
                    Wanted::Nothing
                }
                Err(said) => came(stage, said),
            };
        }
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Scoping { change, chooser };
    Wanted::Nothing
}

/// Over the bars: move, take one, or leave it.
pub(super) fn grading(
    stage: &mut Stage,
    change: &'static Change,
    chosen: Chosen,
    mut chooser: Chooser<Grade>,
    press: &Press,
) -> Wanted {
    match *press {
        Press::Abandon => return Wanted::Nothing,
        Press::Accept => {
            *stage = Stage::Settling {
                change,
                chosen: chosen.graded(chooser.taken().name),
                account: None,
            };
            return Wanted::Nothing;
        }
        Press::Back => chooser.back(),
        Press::Forward => chooser.forward(),
        Press::Typed(_) | Press::Rubout => (),
    }
    *stage = Stage::Grading {
        change,
        chosen,
        chooser,
    };
    Wanted::Nothing
}

/// While what it would cost is with the core: back out, or wait for it.
pub(super) fn costing(stage: &mut Stage, change: &'static Change, press: &Press) -> Wanted {
    if matches!(*press, Press::Abandon) {
        return Wanted::Nothing;
    }
    *stage = Stage::Costing { change };
    Wanted::Nothing
}

/// What the core said it would cost, held for the operator to read and answer.
pub(super) fn costed(change: &'static Change, would: Vec<String>) -> Stage {
    Stage::Settling {
        change,
        chosen: Chosen::nothing(),
        account: Some(Reading::of(would)),
    }
}

/// At the question: move through the account, agree to it, or leave it.
///
/// Only an explicit yes goes ahead, the way the teardown's own question is read and
/// the way every errand is offered. Everything else that is not a move puts the box
/// away and changes nothing.
pub(super) fn settling(
    stage: &mut Stage,
    change: &'static Change,
    chosen: Chosen,
    mut account: Option<Reading>,
    press: &Press,
) -> Wanted {
    if let Some(reading) = account.as_mut() {
        if moved(reading, press) {
            *stage = Stage::Settling {
                change,
                chosen,
                account,
            };
            return Wanted::Nothing;
        }
    }
    if !matches!(*press, Press::Typed('y' | 'Y')) {
        return Wanted::Nothing;
    }
    match change.sent(&chosen, account.is_some()) {
        Ok(command) => {
            *stage = Stage::Applying { change, chosen };
            Wanted::Carry(command)
        }
        Err(said) => came(stage, said),
    }
}

/// While the change is with the core: leaving is the only thing left to ask.
pub(super) fn applying(
    stage: &mut Stage,
    change: &'static Change,
    chosen: Chosen,
    press: &Press,
) -> Wanted {
    *stage = Stage::Applying { change, chosen };
    if super::leaving(press) {
        return Wanted::Leave;
    }
    Wanted::Nothing
}

/// What a change came to: the report, or the caution with the question under it.
///
/// A choice this host could only transcode in software is held rather than recorded,
/// and being held is the one cost the agreement on that action is for. So the caution
/// the core answered with becomes the account, and the question goes under it — the
/// same reading a reset is offered under, arriving here rather than before the first
/// question because whether there is a cost at all is the core's to know.
pub(super) fn applied(
    change: &'static Change,
    chosen: Chosen,
    outcome: &Outcome,
    said: Vec<String>,
) -> Stage {
    if held(outcome) {
        return Stage::Settling {
            change,
            chosen,
            account: Some(Reading::of(said)),
        };
    }
    Stage::Came(Reading::of(said))
}

/// Whether the choice was held rather than recorded.
fn held(outcome: &Outcome) -> bool {
    match outcome {
        Outcome::Quality(report) => matches!(report.disposition, Disposition::Held),
        _ => false,
    }
}

/// A translation that came to no command, said in the words the other surface gives.
fn came(stage: &mut Stage, said: String) -> Wanted {
    *stage = Stage::Came(Reading::of(vec![said]));
    Wanted::Nothing
}

#[cfg(test)]
pub(crate) mod tests;
