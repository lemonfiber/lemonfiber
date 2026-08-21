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
    pub const fn may_act(self) -> bool {
        !matches!(self, Self::ReportOnly)
    }

    /// Whether each repair has to be confirmed one at a time.
    #[must_use]
    pub const fn asks(self) -> bool {
        matches!(self, Self::Ask)
    }
}

/// One repair lemonfiber could carry out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
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
pub fn may_write(recorded: Option<&Record>, holds: Option<&str>) -> Writing {
    match recorded {
        // Nothing recorded: lemonfiber never wrote here, so whatever is there is the
        // operator's own and not a difference from anything lemonfiber intended.
        None => Writing::TheirsAlone,
        Some(record) if record.origin.is_adopted() => Writing::Adopted,
        // Recorded as lemonfiber's own, but the service no longer holds it. Something
        // moved it, and the only hand that reaches a service's settings besides
        // lemonfiber's is the operator's. Reading the origin alone would call this
        // lemonfiber's to overwrite, which is the whole thing this rule exists to stop.
        Some(record) if holds != Some(record.value.as_str()) => Writing::Changed,
        Some(_) => Writing::Ours,
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
mod tests {
    use super::{
        escalation, exhausted, mendable, offerable, offered, Outcome, Repair, Stance, ATTEMPTS,
    };
    use crate::condition::{Condition, Fault};
    use crate::error::{Severity, State};

    /// A raised condition for `check`, downstream of `cause` where one is given.
    fn raised(check: &str, cause: Option<&str>) -> Condition {
        let mut fault = Fault::new(
            "service.stopped",
            Severity::Error,
            "it is not running",
            "start it",
        );
        fault.caused_by = cause.map(str::to_owned);
        Condition::raised(check, &fault, "1000")
    }

    /// A repair answering `check` and nothing else.
    fn repair(check: &str) -> Repair {
        Repair {
            check: check.to_owned(),
            does: "start it again".to_owned(),
            effects: Vec::new(),
            reversible: false,
        }
    }

    /// The names of what would be offered, which is what every case here turns on.
    fn names(repairs: &[Repair], conditions: &[Condition]) -> Vec<String> {
        offered(repairs, &conditions.iter().collect::<Vec<_>>())
            .into_iter()
            .map(|repair| repair.check)
            .collect()
    }

    /// Undoing a repair undoes that repair — not the seed that ran before it, and not the
    /// first-run wizard's writes. The journal is shared, and an operator asking to undo the
    /// thing they just watched happen is not asking for everything lemonfiber ever wrote.
    #[test]
    fn undoing_a_repair_reverses_that_repair_and_nothing_else() {
        use super::{undoing, OPERATION};
        use crate::journal::{Change, Kind, Undo};

        let change = |operation: &str, at: &str, key: &str| Change {
            at: at.to_owned(),
            operation: operation.to_owned(),
            target: "sonarr".to_owned(),
            kind: Kind::Set {
                key: key.to_owned(),
                previous: Some("before".to_owned()),
                current: "after".to_owned(),
            },
        };

        let journal = vec![
            change("seed", "1000", "SEEDED"),
            change(OPERATION, "2000", "FIRST"),
            // The most recent repair, which set two values at once.
            change(OPERATION, "3000", "SECOND"),
            change(OPERATION, "3000", "THIRD"),
        ];

        let put_back = |key: &str| Undo {
            target: "sonarr".to_owned(),
            action: crate::journal::Action::Restore {
                key: key.to_owned(),
                value: Some("before".to_owned()),
            },
        };

        // Both halves of the last repair, most recent first — and neither the seed nor the
        // repair before it.
        assert_eq!(
            undoing(&journal),
            vec![put_back("THIRD"), put_back("SECOND")]
        );

        // Nothing repaired yet is nothing to undo, rather than the whole journal.
        assert!(undoing(&[change("seed", "1000", "SEEDED")]).is_empty());
        assert!(undoing(&[]).is_empty());
    }

    /// The rule that keeps this feature from being something an operator has to defend
    /// their machine against. A value they set is theirs; lemonfiber putting its own back
    /// over it — however sure it is — is what makes people stop trusting a tool that
    /// changes things.
    ///
    /// Which is why the baseline is compared and not merely read. "lemonfiber wrote this
    /// once" and "lemonfiber's value is what is there now" are different claims, and only
    /// the second of them makes a field lemonfiber's to write again.
    #[test]
    fn a_repair_writes_over_what_lemonfiber_wrote_and_nothing_else() {
        use super::{may_write, Writing};
        use crate::baseline::{Origin, Record};

        let recorded = |origin| Record {
            value: "8080".to_owned(),
            at: "1000".to_owned(),
            origin,
        };

        // Its own work, still standing where it left it, put back.
        let ours = may_write(Some(&recorded(Origin::Written)), Some("8080"));
        assert_eq!(ours, Writing::Ours);
        assert!(ours.allowed());
        assert!(ours.refused().is_none());

        // Its own work, but the service no longer holds it. Reading the origin alone
        // would call this lemonfiber's to overwrite; what it actually is, is the edit
        // this rule exists to protect.
        let changed = may_write(Some(&recorded(Origin::Written)), Some("9090"));
        assert_eq!(changed, Writing::Changed);
        assert!(!changed.allowed());
        assert!(changed
            .refused()
            .is_some_and(|remedy| remedy.action.contains("since lemonfiber wrote it")));

        // Cleared rather than changed is still theirs — an emptied field is a decision
        // somebody took, not an absence to fill in.
        assert_eq!(
            may_write(Some(&recorded(Origin::Written)), None),
            Writing::Changed
        );

        // Theirs, adopted — left alone, and the operator told where to go instead. Read
        // before the comparison, so a value they have since moved again stays theirs
        // rather than becoming a change to argue about.
        let adopted = may_write(Some(&recorded(Origin::Adopted)), Some("7070"));
        assert_eq!(adopted, Writing::Adopted);
        assert!(!adopted.allowed());
        assert!(adopted
            .refused()
            .is_some_and(|remedy| remedy.action.contains("you set")));

        // Never written at all: a repair does not start writing somewhere lemonfiber has
        // never been, which is how a fix turns into a surprise.
        let theirs = may_write(None, Some("8080"));
        assert_eq!(theirs, Writing::TheirsAlone);
        assert!(!theirs.allowed());
        assert!(theirs.refused().is_some());
    }

    /// Report-only unless this run said otherwise. A run that changed something because
    /// nobody had said not to is the surprise that costs an operator their trust in
    /// everything else the tool tells them.
    #[test]
    fn nothing_is_acted_on_unless_this_run_said_so() {
        assert_eq!(Stance::default(), Stance::ReportOnly);
        assert!(!Stance::ReportOnly.may_act());
        assert!(Stance::Ask.may_act());
        assert!(Stance::Unattended.may_act());
        assert!(Stance::Ask.asks());
        assert!(!Stance::Unattended.asks());
        assert!(!Stance::ReportOnly.asks());
    }

    /// "It ran" and "it worked" are different claims, and only the second one — proved by
    /// asking the check again — settles anything.
    #[test]
    fn only_a_check_that_passes_afterwards_settles_a_fault() {
        assert!(Outcome::Fixed.settled());
        assert!(!Outcome::FixFailed.settled());
        assert!(!Outcome::Declined.settled());
        assert!(!Outcome::WouldOverwrite.settled());
        assert!(!Outcome::Stopped {
            leaving: "half of it".to_owned()
        }
        .settled());
    }

    /// The classification the operator sees is the one the problem already carried. A
    /// second vocabulary invented here would be a second thing to keep in step.
    #[test]
    fn what_lemonfiber_can_mend_is_what_the_problem_already_said() {
        assert!(mendable(State::Remediable));
        assert!(!mendable(State::Actionable));
        assert!(!mendable(State::Guided));
        assert!(!mendable(State::Unknown));
        assert!(!mendable(State::Suppressed));
    }

    /// Three reasons not to offer one, and each is a different kind of no.
    #[test]
    fn a_repair_is_offered_while_it_is_wanted_and_might_still_work() {
        let mut condition = raised("service.sonarr", None);
        assert!(offerable(&condition));
        assert!(!exhausted(&condition));

        condition.declined = true;
        assert!(
            !offerable(&condition),
            "they said no and it has not been away"
        );
        condition.declined = false;

        condition.attempts = ATTEMPTS;
        assert!(!offerable(&condition));
        assert!(exhausted(&condition), "it has had its chances");
        condition.attempts = 0;

        condition.clear("2000");
        assert!(!offerable(&condition), "nothing to put right");
    }

    /// A fault that comes back deserves the attempt afresh: the count is about lemonfiber
    /// being wrong, and a problem that genuinely returned is a new question.
    #[test]
    fn a_fault_that_comes_back_is_worth_trying_again() {
        let mut condition = raised("service.sonarr", None);
        condition.attempts = ATTEMPTS;
        condition.declined = true;
        condition.clear("2000");

        let fault = Fault::new(
            "service.stopped",
            Severity::Error,
            "it is not running",
            "start it",
        );
        condition.raise(&fault, "3000");

        assert_eq!(condition.attempts, 0);
        assert!(!condition.declined);
        assert!(offerable(&condition));
    }

    /// Beyond what lemonfiber understands is exactly what a support bundle is for, and
    /// somebody who has watched three repairs fail should not also have to work out the
    /// flags — so the command is spelled out rather than described.
    #[test]
    fn a_repair_that_has_run_out_of_chances_hands_over_the_command() {
        let escalating = escalation(&raised("vpn.egress", None));
        assert!(escalating.action.contains("support bundle"));
        assert!(escalating.action.contains("vpn.egress"));
        assert_eq!(
            escalating.detail.as_deref(),
            Some("lemonfiber support --logs 500")
        );
    }

    /// One fault, one repair. A stack whose VPN is down raises a finding for every service
    /// behind it, and six repairs for one fault is how a report stops being read.
    #[test]
    fn findings_sharing_a_cause_are_answered_once() {
        let conditions = vec![
            raised("vpn.up", None),
            raised("service.sonarr", Some("vpn.up")),
            raised("service.radarr", Some("vpn.up")),
        ];
        let repairs = vec![
            repair("vpn.up"),
            repair("service.sonarr"),
            repair("service.radarr"),
        ];

        assert_eq!(names(&repairs, &conditions), vec!["vpn.up".to_owned()]);
    }

    /// Downstream of something *not* being put right this pass, it is on its own — the
    /// cause is only a reason to wait when waiting will actually answer it.
    #[test]
    fn a_dependent_is_offered_where_its_cause_is_not_being_answered() {
        let conditions = vec![raised("service.sonarr", Some("vpn.up"))];
        let repairs = vec![repair("service.sonarr")];

        assert_eq!(
            names(&repairs, &conditions),
            vec!["service.sonarr".to_owned()]
        );
    }

    /// A repair for something nothing has raised is not offered. The conditions are what
    /// say a fault is standing right now; a repair list on its own says nothing.
    #[test]
    fn a_repair_with_nothing_wrong_behind_it_is_not_offered() {
        assert!(names(&[repair("service.sonarr")], &[]).is_empty());
        assert!(names(&[repair("service.sonarr")], &[raised("vpn.up", None)]).is_empty());
    }

    /// The shape a report carries, which is a contract the moment anything reads it: the
    /// outcome is tagged by name, and the state that a stopped repair left behind rides
    /// with it rather than being flattened into a word.
    #[test]
    fn an_outcome_reads_as_what_happened() {
        assert_eq!(
            serde_json::to_string(&Outcome::Fixed).ok(),
            Some(r#"{"outcome":"fixed"}"#.to_owned())
        );
        assert_eq!(
            serde_json::to_string(&Outcome::Stopped {
                leaving: "half of it".to_owned()
            })
            .ok(),
            Some(r#"{"outcome":"stopped","leaving":"half of it"}"#.to_owned())
        );
        assert_eq!(
            serde_json::to_string(&Outcome::FixFailed).ok(),
            Some(r#"{"outcome":"fix_failed"}"#.to_owned())
        );
        assert_eq!(
            serde_json::to_string(&Outcome::Declined).ok(),
            Some(r#"{"outcome":"declined"}"#.to_owned())
        );
        assert_eq!(
            serde_json::to_string(&Outcome::WouldOverwrite).ok(),
            Some(r#"{"outcome":"would_overwrite"}"#.to_owned())
        );
        assert_eq!(
            serde_json::to_string(&Stance::ReportOnly).ok(),
            Some(r#""report_only""#.to_owned())
        );
        assert_eq!(
            serde_json::to_string(&Stance::Ask).ok(),
            Some(r#""ask""#.to_owned())
        );
        assert_eq!(
            serde_json::to_string(&Stance::Unattended).ok(),
            Some(r#""unattended""#.to_owned())
        );
        assert!(serde_json::to_string(&repair("vpn.up"))
            .is_ok_and(|json| json.contains(r#""check":"vpn.up""#)));
    }
}
