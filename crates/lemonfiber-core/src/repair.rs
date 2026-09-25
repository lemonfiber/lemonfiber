//! What lemonfiber can put right itself, and what it would do to put it right.
//!
//! A diagnosis that says what is wrong and leaves the operator to work out the rest is
//! only half of what they came for. This is the other half — but it is the half where
//! being wrong costs them their configuration rather than a wasted minute, so nothing
//! here does anything. It proposes.
//!
//! Everything is stated before it happens and proved after: what a repair would do and
//! what else changes if it does, and then whether the check that raised the finding
//! passes now. A repair that ran without error and left the fault standing is a failure,
//! not a success, and the only way to know the difference is to ask the check again.
//!
//! Pure, like the rest of the model. What a repair *is*, whether it may be offered, which
//! findings it answers at once and when to stop offering it are all decided here, with no
//! service to reach and no file to write; carrying one out happens above.

pub mod run;

use serde::{Deserialize, Serialize};

use crate::baseline::Record;
use crate::condition::Condition;
use crate::error::{Remedy, State};
use crate::journal::{Change, Undo};

pub use crate::ports::service::ASK_FOR_REPAIRS;

/// How many repairs may leave a fault standing before it stops being offered.
///
/// Three, because the first failure can be luck and the second can be a race, and by the
/// third the cause is something lemonfiber has not understood. An operator watching the
/// same repair fail a fourth time is being wasted rather than helped.
pub const ATTEMPTS: u32 = 3;

/// Whether this run may act, and how.
///
/// Report-only unless the operator said otherwise on this very run. A run that changed
/// things because nobody had said not to is the kind of surprise that costs an operator
/// their trust in everything else the tool says.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    /// Say what could be put right, and put none of it right.
    #[default]
    ReportOnly,
    /// Propose each repair, and carry out the ones confirmed.
    Ask,
    /// Carry them out without asking, because this run was told to.
    Unattended,
}

impl Stance {
    /// Whether a repair may be carried out at all under this stance.
    #[must_use]
    pub(crate) const fn may_act(self) -> bool {
        !matches!(self, Self::ReportOnly)
    }

    /// Whether each repair has to be confirmed one at a time.
    #[must_use]
    pub const fn asks(self) -> bool {
        matches!(self, Self::Ask)
    }
}

/// One repair lemonfiber could carry out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Repair {
    /// The check whose finding this answers, as the finding names it.
    pub check: String,
    /// What it would do, in the words the operator will read before confirming.
    pub does: String,
    /// What else changes if it does.
    ///
    /// Stated before it is confirmed and never afterwards, because an effect an operator
    /// learns about after the fact is not something they agreed to. Empty where a repair
    /// touches nothing but the thing it names.
    pub effects: Vec<String>,
    /// Whether carrying it out is recorded well enough to be undone.
    ///
    /// A repair that cannot be reversed is still worth offering — restarting a container
    /// is not undoable and is usually right — but the operator confirming one deserves to
    /// know which kind they are agreeing to.
    pub reversible: bool,
}

/// What an offer was, in a form the consent given for it can name it by.
///
/// A checksum over every word an operator reads before agreeing — what each repair
/// would do, what else changes if it does, whether it can be taken back, and the
/// order they were offered in. Anything that would make the offer read differently
/// makes this read differently, so consent given for one offer cannot be spent on
/// another.
///
/// A surface whose consent crosses a request boundary sends this back with it, and
/// the run that acts recomputes it from a fresh look. That is what a terminal gets
/// for nothing by holding the question open in one process.
///
/// Not a secret and not a signature. It says which offer, not who agreed: this
/// surface's admission is decided above it, once, for every request.
///
/// Named through [`crate::agreement`], which is where the one way of naming a
/// reading lives — a restore's listing is named the same way, and two spellings of
/// the same idea would drift into two answers to the same question.
#[must_use]
pub fn agreement(offered: &[Repair]) -> String {
    let mut words: Vec<&str> = Vec::new();
    for repair in offered {
        words.push(repair.check.as_str());
        words.push(repair.does.as_str());
        words.push(if repair.reversible {
            "reversible"
        } else {
            "irreversible"
        });
        words.extend(repair.effects.iter().map(String::as_str));
    }
    crate::agreement::over(&words)
}

/// What carrying out a repair did, as the mender that carried it out saw it.
///
/// Deliberately not [`Outcome`]. A mender can say what it did; it cannot say whether it
/// worked, because that takes asking the check again. Keeping the two apart in the type
/// system means no mender can report a fault gone on the strength of its own command
/// having succeeded — which is the failure this whole feature exists to avoid.
///
/// There is no refusal here either. Whether a repair would write over something the
/// operator changed by hand is settled before a mender is asked, so that the rule holds
/// for every repair rather than for each one that remembered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attempt {
    /// It ran, and did what it said it would.
    Carried {
        /// What it changed, in the form a reversal reads — enough to put each change
        /// back, and empty where it changed nothing that could be put back.
        ///
        /// Carried by the attempt rather than written by the mender, so the recording is
        /// something a repair *reports* rather than something each mender is trusted to
        /// remember. One place persists them, which is also the place that knows where
        /// the journal lives.
        changes: Vec<Change>,
    },
    /// It stopped partway, leaving this.
    Stopped {
        /// What the machine is now in, said plainly.
        leaving: String,
    },
}

impl Attempt {
    /// Carried out, changing nothing a reversal could read back.
    #[must_use]
    pub const fn carried() -> Self {
        Self::Carried {
            changes: Vec::new(),
        }
    }

    /// Carried out, with what it changed recorded so it can be put back.
    #[must_use]
    pub const fn recorded(changes: Vec<Change>) -> Self {
        Self::Carried { changes }
    }

    /// What this attempt changed, for the caller that persists it.
    #[must_use]
    pub fn changes(&self) -> &[Change] {
        match self {
            Self::Carried { changes } => changes,
            Self::Stopped { .. } => &[],
        }
    }
}

/// How a repair turned out, once the check that raised the finding has been asked again.
///
/// Deliberately not a boolean. "It ran" and "it worked" are different claims, and a model
/// that cannot tell them apart will eventually report the first as the second.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", tag = "outcome")]
#[schemars(rename = "RepairOutcome")]
pub enum Outcome {
    /// It ran, and the check now passes.
    Fixed,
    /// It ran, and the check still fails.
    FixFailed,
    /// It stopped partway, leaving this.
    ///
    /// Named precisely rather than as "failed": a half-applied change is a different
    /// state to be in from an unchanged one, and the operator has to know which they are
    /// looking at before they try anything else.
    Stopped {
        /// What the machine is now in, said plainly.
        leaving: String,
    },
    /// Not carried out, because the operator said no.
    Declined,
    /// Refused, because it would have written over something changed by hand.
    WouldOverwrite,
    /// Not carried out, because the operator declared the area it would write
    /// unmanaged.
    ///
    /// Apart from [`Self::WouldOverwrite`], which is lemonfiber declining to write over
    /// a change it can see. This is lemonfiber obeying an instruction it was given, and
    /// telling somebody the first when they wrote the second would send them looking
    /// for a change they did not make.
    Unmanaged,
}

impl Outcome {
    /// What an attempt amounts to, once the check has been asked again.
    ///
    /// The only place `Fixed` can be constructed from a repair having run, and it takes
    /// `settled` — the check's own answer afterwards — to get there. A repair that
    /// stopped partway is neither fixed nor merely failed however the check now reads:
    /// what matters to the operator is the state it was left in.
    #[must_use]
    pub fn of(attempt: Attempt, settled: bool) -> Self {
        match attempt {
            Attempt::Stopped { leaving } => Self::Stopped { leaving },
            Attempt::Carried { .. } if settled => Self::Fixed,
            Attempt::Carried { .. } => Self::FixFailed,
        }
    }

    /// Whether the fault this answered is gone.
    ///
    /// The one question the report turns on, and the only [`Outcome`] that answers yes is
    /// the one whose check was asked again and passed.
    #[must_use]
    pub const fn settled(&self) -> bool {
        matches!(self, Self::Fixed)
    }
}

/// Whether a finding in this state is something lemonfiber could put right itself.
///
/// The classification the operator sees is the one the problem already carries: a check
/// says what kind of trouble it found when it raises it, and a second vocabulary invented
/// here would be a second thing to keep in step with the first.
#[must_use]
pub const fn mendable(state: State) -> bool {
    matches!(state, State::Remediable)
}

/// Whether this condition should still be offered a repair.
///
/// Three ways it should not be: the operator has declined one and the fault has not been
/// away since; enough repairs have already left it standing; or it is not raised at all.
#[must_use]
pub fn offerable(condition: &Condition) -> bool {
    condition.is_raised() && !condition.declined && !exhausted(condition)
}

/// Whether repairs have been tried on this often enough to stop.
#[must_use]
pub fn exhausted(condition: &Condition) -> bool {
    condition.attempts >= ATTEMPTS
}

/// What to tell an operator whose repair has failed as often as it is going to be tried.
///
/// Not a shrug. The thing that is wrong is beyond what lemonfiber understands, which is
/// exactly the situation a support bundle exists for, so the way to ask somebody is the
/// remedy — with the command spelled out rather than described, because a person who has
/// watched three repairs fail should not also have to work out the flags.
#[must_use]
pub fn escalation(condition: &Condition) -> Remedy {
    Remedy::new(format!(
        "Ask for help with a support bundle — {} has not been put right by {ATTEMPTS} attempts",
        condition.check
    ))
    .with_detail("lemonfiber support --logs 500")
}

/// What a repair records itself as in the change journal.
///
/// Named so that undoing one can find its own work: the journal is shared with seeding and
/// the first-run wizard, and an operator undoing a repair must not have those unwound under
/// them.
pub const OPERATION: &str = "repair";

/// The undos that reverse the most recent repair, and nothing else.
///
/// Scoped to one repair rather than to the whole journal, because "undo everything
/// lemonfiber has ever written" is not what somebody means by undoing the thing they just
/// watched happen. Everything that repair changed goes back — a repair that set two values
/// half-undone is a worse state than either the one before it or the one after.
///
/// Most recent first, for the reason [`crate::journal::Journal::rewind`] is: a later change
/// may rest on an earlier one.
#[must_use]
pub fn undoing(changes: &[Change]) -> Vec<Undo> {
    let Some(last) = changes
        .iter()
        .rev()
        .find(|change| change.operation == OPERATION)
    else {
        return Vec::new();
    };
    changes
        .iter()
        .rev()
        .filter(|change| change.operation == OPERATION && change.at == last.at)
        .map(Change::undo)
        .collect()
}

/// Whether a repair may write over what is there, and why not where it may not.
///
/// The one rule that keeps auto-remediation from being something an operator has to defend
/// their machine against. A value they set by hand is theirs, and lemonfiber pushing its
/// own over it — however sure it is — is the behaviour that makes people stop trusting a
/// tool that changes things.
///
/// Read from the baseline rather than guessed at: it records what lemonfiber wrote and what
/// it adopted from the operator, and those are different claims about the same field.
///
/// And compared with `holds` — what the service has now — rather than read alone. "lemonfiber
/// wrote this once" and "lemonfiber's value is what is there" are different claims too, and
/// only the second of them makes a field lemonfiber's to write again. A field it wrote and
/// somebody has since changed reads as theirs, whoever wrote it first.
#[must_use]
pub(crate) fn may_write(recorded: Option<&Record>, holds: Option<&str>) -> Writing {
    match recorded {
        // Nothing recorded: lemonfiber never wrote here, so whatever is there is the
        // operator's own and not a difference from anything lemonfiber intended.
        None => Writing::TheirsAlone,
        Some(record) if record.origin.is_adopted() => Writing::Adopted,
        Some(record) => may_put_back(&record.value, holds),
    }
}

/// Whether lemonfiber's own value is still the one that is there, and so still
/// lemonfiber's to write.
///
/// The comparison at the heart of [`may_write`], apart from it so that reversing a
/// repair asks the same question rather than a second one that agrees with it by
/// coincidence. A reversal reads no baseline — the journal already says what the
/// repair put there — but the rule it has to keep is the same one: a value that is
/// no longer what lemonfiber wrote has been moved by the only other hand that
/// reaches it, and is theirs now whoever wrote it first.
///
/// Reading `wrote` alone would call every one of those lemonfiber's to write over,
/// which is the whole thing this rule exists to stop.
#[must_use]
pub(crate) fn may_put_back(wrote: &str, holds: Option<&str>) -> Writing {
    if holds == Some(wrote) {
        Writing::Ours
    } else {
        Writing::Changed
    }
}

/// What the baseline says about a field a repair would write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Writing {
    /// lemonfiber wrote this value, so putting it back is restoring its own work.
    Ours,
    /// lemonfiber wrote it and the operator has since changed it in the service. Theirs
    /// now, whoever wrote it first.
    Changed,
    /// The operator set it and lemonfiber adopted it. Theirs to keep.
    Adopted,
    /// Nothing was ever recorded here, so nothing lemonfiber knows about is being changed.
    TheirsAlone,
    /// The operator declared this area unmanaged, so it is not lemonfiber's to write
    /// whatever the baseline says about who wrote what.
    ///
    /// The one answer here that is not read off a record of what happened: the other
    /// three are conclusions about a value, and this is an instruction about an area.
    /// Kept apart from them because the sentence an operator is owed differs — being
    /// told lemonfiber would have overwritten a change sends somebody looking for a
    /// change they did not make.
    Unmanaged,
}

impl Writing {
    /// Whether a repair may go ahead.
    #[must_use]
    pub const fn allowed(self) -> bool {
        matches!(self, Self::Ours)
    }

    /// Why it may not, for an operator who asked to have it put right.
    ///
    /// Deferred to drift rather than answered here: what to do about a value the operator
    /// owns is a question that subsystem exists for, and a repair that argued with it would
    /// be a second opinion about the same field.
    #[must_use]
    pub fn refused(self) -> Option<Remedy> {
        match self {
            Self::Ours => None,
            Self::Changed => Some(
                Remedy::new("You have changed this since lemonfiber wrote it, so it is left alone")
                    .with_detail("lemonfiber reset --confirm restores lemonfiber's own"),
            ),
            Self::Adopted => Some(
                Remedy::new("This is a value you set and lemonfiber adopted, so it is left alone")
                    .with_detail("lemonfiber reset --confirm restores lemonfiber's own"),
            ),
            Self::TheirsAlone => Some(
                Remedy::new("lemonfiber has never written this, so it will not start now")
                    .with_detail("lemonfiber seed writes what is missing"),
            ),
            // The setting is named through its own constant rather than spelled here,
            // so a rename reaches this sentence and an operator is never told to read
            // back something by a name nothing answers to.
            Self::Unmanaged => Some(
                Remedy::new(
                    "You declared this unmanaged, so lemonfiber observes it and writes \
                             nothing",
                )
                .with_detail(format!(
                    "lemonfiber config show {} reads back what you declared",
                    crate::config::UNMANAGED_KEY
                )),
            ),
        }
    }
}

/// The repairs worth offering, once what has been declined, exhausted or already answered
/// by another repair is taken out.
///
/// Findings that share a cause are answered once. A stack whose VPN is down raises a
/// finding for every service behind it, and offering the operator six repairs for one
/// fault is how a report stops being read — so a repair whose condition is caused by
/// something else being repaired in the same pass is left out, and re-verified afterwards
/// through the check it came from rather than through a repair of its own.
#[must_use]
pub fn offered(repairs: &[Repair], conditions: &[&Condition]) -> Vec<Repair> {
    let standing: Vec<&&Condition> = conditions
        .iter()
        .filter(|condition| offerable(condition))
        .collect();
    let answering: Vec<&str> = repairs
        .iter()
        .filter(|repair| {
            standing
                .iter()
                .any(|standing| standing.check == repair.check)
        })
        .map(|repair| repair.check.as_str())
        .collect();

    repairs
        .iter()
        .filter(|repair| {
            let Some(condition) = standing
                .iter()
                .find(|standing| standing.check == repair.check)
            else {
                return false;
            };
            // Downstream of something else this pass will put right: left out here and
            // re-verified afterwards, since the cause going away may take it with it.
            condition
                .caused_by
                .as_deref()
                .is_none_or(|cause| !answering.contains(&cause))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
