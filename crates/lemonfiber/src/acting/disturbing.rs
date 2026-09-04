//! The runs that disturb something to answer, asked for under the reads that do not.
//!
//! Two of this screen's questions are a word wearing two requests. Most of what each
//! answers looks and touches nothing, and this screen asks that as a question like any
//! other read. Widened, each reaches past this machine.
//!
//! `doctor` widens into two checks that touch: one takes the tunnel away to prove the
//! killswitch brings the traffic down with it, and one spends a live search against the
//! indexers, counted towards the daily allowance they hold the operator to.
//!
//! `trace` widens into the one thing a trace can do that is not a read. An item somebody
//! is asking for that nothing has been grabbed for stopped for one of two reasons, and no
//! service in this stack can tell them apart from its own records: the indexers carry
//! nothing for it, or they carry releases the quality in force rejects. Only asking the
//! indexers settles it, and that asking spends a search against the same allowance.
//!
//! **Neither is a question.** Every question on this screen goes through
//! [`lemonfiber_api::reads`], and a read that disturbed something would not be a read —
//! the argument the web surface settled this on. So each widened run goes through the
//! table of *actions*, by the names `diagnose` and `search`, which are the same names and
//! the same required word a browser sends.
//!
//! **Neither is an errand either.** The list behind `m` has one rule: an errand that
//! carries an agreement answers, unconfirmed, with what it would do and changes nothing,
//! which is what makes that run the account its question sits under. These have no
//! unconfirmed half at all — the widening is required, because a run that disturbs nothing
//! is the read already offered — so joining them to that list would make the list's rule
//! untrue for the six it was written for. That is the argument [`super::quality`] already
//! made and won for the three writes that sit apart from it.
//!
//! **And neither is worth a key.** Two actions, on a screen that deliberately refused a
//! letter per errand. What each is, instead, is the second half of an answer the operator
//! is already reading: an ordinary diagnosis reports both disturbing checks as unverified
//! and each of those findings says to run *that one* with the widening, and an ordinary
//! trace says in as many words that whether the indexers carry nothing or the quality in
//! force wants none of what they carry is not known. So the offer sits under the answer
//! that asked for it, and the account this consequence is put under is not a rehearsal —
//! it is the report that named the gap.
//!
//! **The narrowing is what the reading was narrowed by.** A diagnosis asked whole is
//! widened whole; one asked about a family of checks is widened over that family and no
//! other. A trace is widened over the show it followed, and over the one season it was
//! narrowed to where it was narrowed to one. Nothing is typed twice and no list of
//! families or shows is written down here: the words that narrowed the read are the words
//! that narrow the widening, which is what keeps an operator following
//! "run `--only services.releases --disruptive`" from dropping the tunnel as well.
//!
//! **Only what comes to a command is offered.** Each widening goes through the same
//! translation the reading went through, and where it reaches none there is no offer under
//! the answer — the rule the five actions on their own keys build their subjects by,
//! rather than a refusal produced after somebody has agreed to something.

use lemonfiber_api::actions::{named, Arguments, Disturbing};
use lemonfiber_api::reads::{CHECKS, TRACE};
use lemonfiber_core::app::Command;

use super::question::Question;
use super::{Press, Stage, Wanted};

/// One read whose answer carries an offer to ask the same thing again, at a cost.
pub(crate) struct Widened {
    /// The read the answer under the offer was asked at.
    read: &'static str,
    /// The name every surface calls the widened run by.
    pub(crate) action: &'static str,
    /// What the widened run is called while it is with the core.
    pub(crate) name: &'static str,
    /// The question, put under the answer that has just been read.
    pub(crate) asks: &'static str,
    /// What that comes to, in the line under the question.
    pub(crate) about: &'static str,
    /// The widened run's own arguments, filled from the words the read was narrowed by.
    ///
    /// A function rather than a field per argument, because the two reads narrow by
    /// different things: a diagnosis by a family of checks, a trace by a title and a
    /// season. What they share is that the words are the reading's, never a second set
    /// typed for the widening.
    fills: fn(&[String]) -> Arguments,
}

/// The diagnosis, widened to the checks that disturb a running system.
pub(crate) static DIAGNOSIS: Widened = Widened {
    read: CHECKS,
    action: "diagnose",
    name: "the checks that disturb",
    asks: "Run these checks again, including the ones that disturb",
    about: "it takes the tunnel away to prove the killswitch, and spends a live indexer search",
    fills: narrowing,
};

/// The trace, widened to asking the indexers what they carry for the item it followed.
static SEARCH: Widened = Widened {
    read: TRACE,
    action: "search",
    name: "the search against the indexers",
    asks: "Ask the indexers what they carry for this",
    about: "it tells nothing at your quality from nothing at all, and spends a live search",
    fills: following,
};

/// The reads whose answers carry one, in the order the questions behind `a` read them.
static OFFERED_UNDER: &[&Widened] = &[&DIAGNOSIS, &SEARCH];

/// Every widened run this screen offers, in the order the answers carrying them read.
#[cfg(test)]
pub(super) fn every() -> impl Iterator<Item = &'static Widened> {
    OFFERED_UNDER.iter().copied()
}

/// A diagnosis's own arguments: the family the reading was narrowed by, and the word
/// that makes it the run which disturbs.
///
/// The first word and not a search for one. A question over that read is given a family
/// or given nothing, so the first word it was given is the word that narrowed it — and
/// there is never a second.
fn narrowing(said: &[String]) -> Arguments {
    Arguments {
        disruptive: Disturbing::Included,
        only: said.first().filter(|word| !word.is_empty()).cloned(),
        ..Arguments::default()
    }
}

/// A trace's own arguments: what was followed, the season it was narrowed to where it
/// was, and the word that makes it the run which asks the indexers.
///
/// The season is taken as a number here although the read took it as written, because
/// this is past the point at which a word that is not a season could have got in: the
/// read it is carried over from parsed that same word and answered.
fn following(said: &[String]) -> Arguments {
    Arguments {
        disruptive: Disturbing::Included,
        term: said.first().filter(|word| !word.is_empty()).cloned(),
        season: said.get(1).and_then(|word| word.parse().ok()),
        ..Arguments::default()
    }
}

/// A widened run offered under an answer.
///
/// The command rather than the words it was built from: it has already been through
/// the web surface's table of actions by the time there is anything to offer, so
/// what the operator agrees to is the request that will be sent and not a second
/// description of it.
pub(crate) struct Widening {
    /// The run, as the table of actions named it.
    command: Command,
    /// Which widened run it is, so the question above it and the foot of the screen
    /// under it name what is being agreed to rather than the only one there once was.
    widened: &'static Widened,
}

impl Widening {
    /// What is being offered, for the words the screen puts around it.
    pub(crate) const fn widened(&self) -> &'static Widened {
        self.widened
    }
}

/// The widening offered under an answer, or nothing where the answer carries none.
///
/// Held by the read the question was asked at rather than by the shape of what came
/// back. The two are the same fact — only a question over one of these reads reaches
/// the command that answers it — and the read is the half this screen chose, which
/// makes it the half worth asking.
pub(super) fn under(question: &Question, said: &[String]) -> Option<Widening> {
    let widened = OFFERED_UNDER
        .iter()
        .copied()
        .find(|offered| offered.read == question.read)?;
    named(widened.action, (widened.fills)(said))
        .ok()
        .map(|command| Widening { command, widened })
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
    *stage = Stage::Disturbing(widening.widened);
    Wanted::Carry(widening.command)
}

/// While the widened run is with the core: leaving is the only thing left to ask.
///
/// Nothing is drawn over the panels while it runs, which is not the general rule
/// arriving by accident: one of these is proving that traffic stops when the tunnel
/// goes, and the panel that says where traffic leaves from is behind this box.
/// Covering it would take away the one thing worth watching.
pub(super) fn disturbing(stage: &mut Stage, widened: &'static Widened, press: &Press) -> Wanted {
    *stage = Stage::Disturbing(widened);
    if super::leaving(press) {
        return Wanted::Leave;
    }
    Wanted::Nothing
}

#[cfg(test)]
mod tests {
    use super::{answered, disturbing, under, Widening, DIAGNOSIS, OFFERED_UNDER};
    use crate::acting::question::tests::called;
    use crate::acting::{Press, Stage, Wanted};
    use lemonfiber_api::actions::OFFERED as WEB;
    use lemonfiber_core::app::Command;
    use lemonfiber_core::doctor::{Category, Narrowing};

    /// The widened run one answer offers, or nothing where it offers none.
    fn offered(question: &str, typed: &[&str]) -> Option<Widening> {
        let said: Vec<String> = typed.iter().map(|word| (*word).to_owned()).collect();
        under(called(question), &said)
    }

    /// The command one answer's offer would send.
    fn sends(question: &str, typed: &[&str]) -> Option<Command> {
        offered(question, typed).map(|widening| widening.command)
    }

    /// Which widened run a stage is holding, or nothing for a stage holding none.
    fn running(stage: &Stage) -> Option<&'static str> {
        match *stage {
            Stage::Disturbing(widened) => Some(widened.action),
            _ => None,
        }
    }

    /// The whole point of naming the action rather than assembling a command here:
    /// every widened run has to be something another surface already offers, or the
    /// requirement this screen is built for is defeated by the thing built for it.
    #[test]
    fn every_widened_run_is_an_action_the_other_surfaces_offer() {
        for widened in OFFERED_UNDER {
            assert!(WEB.contains(&widened.action), "{}", widened.action);
        }
        assert_eq!(OFFERED_UNDER.len(), 2);
    }

    /// A diagnosis asked whole is widened whole, and the widening is carried rather
    /// than assumed — a run without it is the read that was just answered.
    #[test]
    fn a_diagnosis_asked_whole_is_widened_over_the_whole_suite() {
        assert_eq!(
            sends("how this stack is doing", &[""]),
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
            sends("one family of checks", &["vpn"]),
            Some(Command::Doctor {
                narrowing: Narrowing::Category(Category::Vpn),
                disruptive: true,
                accept: None,
            })
        );
        assert_eq!(
            sends("one family of checks", &["services.releases"]),
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
        assert!(sends("one family of checks", &["nonsense"]).is_none());
        // A trace with nothing typed is refused by the same table, for the reason the
        // read refuses it: a trace with no subject follows nothing.
        assert!(sends("where one thing is", &[""]).is_none());
    }

    /// A trace is widened over the show it followed, and the widened run is the search
    /// against the indexers rather than the read that was just answered.
    #[test]
    fn a_trace_is_widened_into_the_search_that_asks_the_indexers() {
        assert_eq!(
            sends("where one thing is", &["The Expanse"]),
            Some(Command::Trace {
                term: "The Expanse".to_owned(),
                season: None,
                searching: true,
            })
        );
    }

    /// The season the reading was narrowed to narrows the search too — carried over
    /// rather than dropped, or an operator asking about one season would spend the
    /// search on a report about every season there is.
    #[test]
    fn a_trace_narrowed_to_one_season_searches_for_that_season() {
        assert_eq!(
            sends("where one season of it is", &["The Expanse", "2"]),
            Some(Command::Trace {
                term: "The Expanse".to_owned(),
                season: Some(2),
                searching: true,
            })
        );
    }

    /// An answer over a read that carries no widening offers nothing, or a `y` over a
    /// setting would take the tunnel away.
    #[test]
    fn an_answer_over_a_read_with_no_widening_offers_none() {
        assert!(sends("settings", &[""]).is_none());
        assert!(sends("what to watch on", &[]).is_none());
    }

    /// Each question says what its own run costs before it costs it, which is the
    /// whole of what an operator has to decide on.
    #[test]
    fn each_question_names_what_its_run_will_spend() {
        for widened in OFFERED_UNDER {
            assert!(!widened.name.is_empty(), "{}", widened.action);
            assert!(!widened.asks.is_empty(), "{}", widened.action);
            assert!(widened.about.contains("search"), "{}", widened.about);
        }
        assert!(DIAGNOSIS.asks.contains("disturb"), "{}", DIAGNOSIS.asks);
        assert!(DIAGNOSIS.about.contains("tunnel"), "{}", DIAGNOSIS.about);
    }

    /// Only an explicit yes goes ahead, and everything else puts the answer away.
    #[test]
    fn only_an_explicit_yes_runs_the_widened_diagnosis() {
        let mut stage = Stage::Idle;

        assert_eq!(
            answered(
                &mut stage,
                offered("how this stack is doing", &[""]),
                &Press::Typed('n')
            ),
            Wanted::Nothing
        );
        assert!(matches!(stage, Stage::Idle));

        let wanted = answered(
            &mut stage,
            offered("how this stack is doing", &[""]),
            &Press::Typed('Y'),
        );

        assert!(matches!(wanted, Wanted::Carry(Command::Doctor { .. })));
        assert_eq!(running(&stage), Some("diagnose"));
    }

    /// The yes over a trace sends the search, and the stage carries which run it is —
    /// so the foot of the screen names the search rather than the checks.
    #[test]
    fn a_yes_over_a_trace_sends_the_search_and_says_which_run_it_is() {
        let mut stage = Stage::Idle;

        let wanted = answered(
            &mut stage,
            offered("where one thing is", &["The Expanse"]),
            &Press::Typed('y'),
        );

        assert!(matches!(wanted, Wanted::Carry(Command::Trace { .. })));
        assert_eq!(running(&stage), Some("search"));
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
        let mut stage = Stage::Disturbing(&DIAGNOSIS);

        assert_eq!(
            disturbing(&mut stage, &DIAGNOSIS, &Press::Forward),
            Wanted::Nothing
        );
        assert_eq!(running(&stage), Some("diagnose"));

        assert_eq!(
            disturbing(&mut stage, &DIAGNOSIS, &Press::Typed('q')),
            Wanted::Leave
        );
        assert_eq!(running(&stage), Some("diagnose"));
    }
}
