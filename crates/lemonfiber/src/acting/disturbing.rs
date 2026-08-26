//! The diagnosis that disturbs a running system, asked for under the one that does
//! not.
//!
//! `doctor` is two requests wearing one word. Most of it looks and touches nothing,
//! and this screen asks that as a question like any other read. Two of its checks do
//! touch: one takes the tunnel away to prove the killswitch brings the traffic down
//! with it, and one spends a live search against the indexers, counted towards the
//! daily allowance they hold the operator to. Those run only where somebody asked
//! for them, and asking is a write.
//!
//! **It is not a question.** Every question on this screen goes through
//! [`lemonfiber_api::reads`], and a read that disturbed something would not be a
//! read — the argument the web surface settled this on. So the widened run goes
//! through the table of *actions*, by the name `diagnose`, which is the same name
//! and the same required word a browser sends.
//!
//! **It is not an errand either.** The list behind `m` has one rule: an errand that
//! carries an agreement answers, unconfirmed, with what it would do and changes
//! nothing, which is what makes that run the account its question sits under. This
//! action has no unconfirmed half at all — the widening is required, because a
//! diagnosis that disturbs nothing is the read already offered — so joining it to
//! that list would make the list's rule untrue for the six it was written for. That
//! is the argument [`super::quality`] already made and won for the three writes that
//! sit apart from it.
//!
//! **And it is not worth a key.** One action, on a screen that deliberately refused
//! a letter per errand. What it is, instead, is the second half of an answer the
//! operator is already reading: an ordinary diagnosis reports both disturbing checks
//! as unverified, and each of those findings says to run *that one* with the
//! widening. So the offer sits under the answer that asked for it, and the account
//! this consequence is put under is not a rehearsal — it is the report that named
//! the gap.
//!
//! **The narrowing is what the reading was narrowed by.** A diagnosis asked whole is
//! widened whole; one asked about a family of checks is widened over that family and
//! no other. Nothing is typed twice and no list of families is written down here: the
//! word that narrowed the read is the word that narrows the widening, which is what
//! keeps an operator following "run `--only services.releases --disruptive`" from
//! dropping the tunnel as well.
//!
//! **Only what comes to a command is offered.** The widening goes through the same
//! translation the reading went through, and where it reaches none there is no offer
//! under the answer — the rule the five actions on their own keys build their
//! subjects by, rather than a refusal produced after somebody has agreed to
//! something.

use lemonfiber_api::actions::{named, Arguments, Disturbing};
use lemonfiber_api::reads::CHECKS;
use lemonfiber_core::app::Command;

use super::question::Question;
use super::{Press, Stage, Wanted};

/// The name every surface calls this action by.
pub(crate) const ACTION: &str = "diagnose";

/// What this run is called while it is with the core.
pub(crate) const NAME: &str = "the checks that disturb";

/// The question, put under the diagnosis that has just been read.
pub(crate) const ASKS: &str = "Run these checks again, including the ones that disturb";

/// What that comes to, in the line under the question.
pub(crate) const ABOUT: &str =
    "it takes the tunnel away to prove the killswitch, and spends a live indexer search";

/// A widened run of the checks an answer on the screen has just reported.
///
/// The command rather than the words it was built from: it has already been through
/// the web surface's table of actions by the time there is anything to offer, so
/// what the operator agrees to is the request that will be sent and not a second
/// description of it.
pub(crate) struct Widening {
    /// The run, as the table of actions named it.
    command: Command,
}

/// The widening offered under an answer, or nothing where the answer is not a
/// diagnosis.
///
/// Held by the read the question was asked at rather than by the shape of what came
/// back. The two are the same fact — only a question over [`CHECKS`] reaches
/// `Command::Doctor`, and only that command answers with a diagnosis — and the read
/// is the half this screen chose, which makes it the half worth asking.
pub(super) fn under(question: &Question, said: &[String]) -> Option<Widening> {
    if question.read != CHECKS {
        return None;
    }
    // The first word and not a search for one. A question over this read is given a
    // family or given nothing, so the first word it was given is the word that
    // narrowed it — and there is never a second.
    let narrowed = said.first().filter(|word| !word.is_empty());
    let given = Arguments {
        disruptive: Disturbing::Included,
        only: narrowed.cloned(),
        ..Arguments::default()
    };
    named(ACTION, given)
        .ok()
        .map(|command| Widening { command })
}

/// At the answer: agree to the widened run, or put the answer away.
///
/// Only an explicit yes goes ahead, the way every other question on this screen is
/// read. An answer with no widening under it answers nothing here — a reading of
/// what one person asked for is not something a `y` should set running.
pub(super) fn answered(stage: &mut Stage, widening: Option<Widening>, press: &Press) -> Wanted {
    let Some(widening) = widening else {
        return Wanted::Nothing;
    };
    if !matches!(*press, Press::Typed('y' | 'Y')) {
        return Wanted::Nothing;
    }
    *stage = Stage::Disturbing;
    Wanted::Carry(widening.command)
}

/// While the widened run is with the core: leaving is the only thing left to ask.
///
/// Nothing is drawn over the panels while it runs, which is not the general rule
/// arriving by accident: the check being run is whether traffic stops when the
/// tunnel goes, and the panel that says where traffic leaves from is behind this
/// box. Covering it would take away the one thing worth watching.
pub(super) fn disturbing(stage: &mut Stage, press: &Press) -> Wanted {
    *stage = Stage::Disturbing;
    if super::leaving(press) {
        return Wanted::Leave;
    }
    Wanted::Nothing
}

#[cfg(test)]
mod tests {
    use super::{answered, disturbing, under, Widening, ABOUT, ACTION, ASKS, NAME};
    use crate::acting::question::tests::called;
    use crate::acting::{Press, Stage, Wanted};
    use lemonfiber_api::actions::OFFERED as WEB;
    use lemonfiber_core::app::Command;
    use lemonfiber_core::doctor::{Category, Narrowing};

    /// The widened run one answer offers, or nothing where it offers none.
    fn offered(question: &str, typed: &str) -> Option<Widening> {
        under(called(question), &[typed.to_owned()])
    }

    /// The command one answer's offer would send.
    fn sends(question: &str, typed: &str) -> Option<Command> {
        offered(question, typed).map(|widening| widening.command)
    }

    /// The whole point of naming the action rather than assembling a command here:
    /// the widened run has to be something another surface already offers, or the
    /// requirement this screen is built for is defeated by the thing built for it.
    #[test]
    fn the_widened_run_is_an_action_the_other_surfaces_offer() {
        assert!(WEB.contains(&ACTION), "{ACTION}");
    }

    /// A diagnosis asked whole is widened whole, and the widening is carried rather
    /// than assumed — a run without it is the read that was just answered.
    #[test]
    fn a_diagnosis_asked_whole_is_widened_over_the_whole_suite() {
        assert_eq!(
            sends("how this stack is doing", ""),
            Some(Command::Doctor {
                narrowing: Narrowing::Suite,
                disruptive: true,
                accept: None,
            })
        );
    }

    /// The claim this slice turns on: the word that narrowed the reading is the word
    /// that narrows the widening. An operator following a finding that says to run
    /// one family would otherwise drop the tunnel to spend one indexer search.
    #[test]
    fn a_diagnosis_asked_about_one_family_is_widened_over_that_family_alone() {
        assert_eq!(
            sends("one family of checks", "vpn"),
            Some(Command::Doctor {
                narrowing: Narrowing::Category(Category::Vpn),
                disruptive: true,
                accept: None,
            })
        );
        assert_eq!(
            sends("one family of checks", "services.releases"),
            Some(Command::Doctor {
                narrowing: Narrowing::Check("services.releases".to_owned()),
                disruptive: true,
                accept: None,
            })
        );
    }

    /// Nothing this screen could not send is offered. A word the table of actions
    /// refuses leaves no offer under the answer, rather than a refusal produced after
    /// somebody has agreed to something.
    #[test]
    fn a_narrowing_that_reaches_no_command_is_not_offered_at_all() {
        assert!(sends("one family of checks", "nonsense").is_none());
    }

    /// An answer that is not a diagnosis offers nothing, or a `y` over a trace would
    /// take the tunnel away.
    #[test]
    fn an_answer_that_is_not_a_diagnosis_offers_no_widening() {
        assert!(sends("where one thing is", "The Expanse").is_none());
        assert!(sends("settings", "").is_none());
    }

    /// The question says what it costs before it costs it, which is the whole of
    /// what an operator has to decide on.
    #[test]
    fn the_question_names_what_the_run_will_disturb() {
        assert!(ASKS.contains("disturb"));
        assert!(ABOUT.contains("tunnel"), "{ABOUT}");
        assert!(ABOUT.contains("search"), "{ABOUT}");
        assert!(!NAME.is_empty());
    }

    /// Only an explicit yes goes ahead, and everything else puts the answer away.
    #[test]
    fn only_an_explicit_yes_runs_the_checks_that_disturb() {
        let mut stage = Stage::Idle;

        assert_eq!(
            answered(
                &mut stage,
                offered("how this stack is doing", ""),
                &Press::Typed('n')
            ),
            Wanted::Nothing
        );
        assert!(matches!(stage, Stage::Idle));

        let wanted = answered(
            &mut stage,
            offered("how this stack is doing", ""),
            &Press::Typed('Y'),
        );

        assert!(matches!(wanted, Wanted::Carry(Command::Doctor { .. })));
        assert!(matches!(stage, Stage::Disturbing));
    }

    /// An answer with no widening under it answers nothing to a `y`.
    #[test]
    fn a_yes_over_an_answer_with_no_offer_under_it_sends_nothing() {
        let mut stage = Stage::Idle;

        assert_eq!(
            answered(&mut stage, None, &Press::Typed('y')),
            Wanted::Nothing
        );

        assert!(matches!(stage, Stage::Idle));
    }

    /// While it runs, leaving is the only thing left to ask — and everything else
    /// leaves it running.
    #[test]
    fn a_run_that_disturbs_is_left_rather_than_stopped() {
        let mut stage = Stage::Disturbing;

        assert_eq!(disturbing(&mut stage, &Press::Forward), Wanted::Nothing);
        assert!(matches!(stage, Stage::Disturbing));

        assert_eq!(disturbing(&mut stage, &Press::Typed('q')), Wanted::Leave);
        assert!(matches!(stage, Stage::Disturbing));
    }
}
