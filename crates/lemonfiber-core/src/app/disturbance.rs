//! What an operation takes away while it runs, said before it takes it.
//!
//! An operator weighing a stop is weighing being without something for a while,
//! and "a while" is the part they cannot look up. A second is a shrug and three
//! minutes is a different decision — and the decision is made before the command
//! runs, which is the only moment saying so is any use.
//!
//! Every length here is one the run is actually held to, never a guess at one. A
//! duration worked out by a surface would be a guess at something this side
//! knows, wrong in exactly the cases somebody most needs it, and wrong silently,
//! because nothing on either side would ever compare it to what happened. So an
//! operation with no clock on it says that instead, and says what it is waiting
//! for.

use std::time::Duration;

use super::engine::Waiting;
use super::{Command, Ctx};

/// What the container engine gives a service to stop in.
///
/// Compose's own default, and named here because it is the length a stop is
/// actually held to rather than an estimate of one. It is not imposed: passing a
/// timeout would override a service that asked for a longer one, and a service
/// flushing to disk asked for that length for a reason. The day the stack
/// declares its own, this becomes the longest of them rather than one number.
const GRACE: Duration = Duration::from_secs(10);

/// How long an operation takes something away for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disturbance {
    /// It ends, and this is the length it is held to.
    Bounded(Duration),
    /// It does not end on a clock, and this is what it is waiting for.
    Until(Awaiting),
}

/// What an operation with no bound on it is waiting for.
///
/// A case rather than a sentence, so what an operation is waiting for is a value
/// something can be asked about. The words are built from it in one place below;
/// a phrase carried here instead would be a phrase every later surface had to
/// parse back into the fact it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Awaiting {
    /// Everything still coming down has finished arriving.
    Downloads,
}

/// One of the ways a lifecycle verb puts the stack out.
///
/// Between the command and the length is a situation: several commands put the
/// stack in the same one, and two surfaces spell the same command differently.
/// Naming the situation gives both the length to read off — [`of`] maps a command
/// onto it, [`everything`] lists every one of them — so a verb's bound and a
/// payload's bound are one answer rather than two that agree today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Situation {
    /// Services are being brought up.
    Starting,
    /// Services are being taken down, interrupting anything still arriving.
    Stopping,
    /// The stack is being taken down once everything still arriving has landed.
    StoppingAfterDownloads,
    /// Services are being restarted.
    Restarting,
    /// The running set is being changed to a different one.
    Switching,
}

impl Situation {
    /// Every situation there is.
    ///
    /// A payload that states all of them reads this rather than listing them a
    /// second time. Kept honest by [`Situation::called`], whose match has no
    /// wildcard: a new situation fails to compile there, and the test that reads
    /// both then refuses any payload that has not grown a field for it.
    pub const EVERY: &[Self] = &[
        Self::Starting,
        Self::Stopping,
        Self::StoppingAfterDownloads,
        Self::Restarting,
        Self::Switching,
    ];

    /// What a payload calls this situation.
    ///
    /// The field name, so that the list above and the shape on the wire can be
    /// held against each other by something that reads them both.
    #[must_use]
    pub const fn called(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Stopping => "stopping",
            Self::StoppingAfterDownloads => "stopping_after_downloads",
            Self::Restarting => "restarting",
            Self::Switching => "switching",
        }
    }

    /// How long this situation lasts.
    ///
    /// The patience is handed in because it is a knob: an operator on a slow disk
    /// runs with a longer one, and a length stated from a constant would be the
    /// wrong length for exactly the run that needed the knob.
    #[must_use]
    pub const fn takes(self, patience: Duration) -> Disturbance {
        match self {
            // Starting is bounded by how long the run waits for services to
            // settle, which is the same knob that decides when it gives up. A
            // start that has to fetch an image spends that time before this one
            // begins, and it is narrated as it happens rather than promised in
            // advance.
            Self::Starting | Self::Restarting | Self::Switching => Disturbance::Bounded(patience),
            Self::Stopping => Disturbance::Bounded(GRACE),
            // A teardown told to let the downloads finish is the one operation
            // here with nothing to bound it: what it waits for is a torrent at
            // ninety-four per cent, and how long that takes belongs to whoever
            // is seeding it.
            Self::StoppingAfterDownloads => Disturbance::Until(Awaiting::Downloads),
        }
    }
}

/// What this command disturbs, or nothing where this does not yet say.
///
/// `None` means **unstated**, not *nothing*. The verbs that start and stop
/// services are the ones answered here; setup, an uninstall, a restore and a
/// disruptive diagnosis all take something away too, and what bounds each of
/// them is a different subsystem's answer. Saying `None` for them is this side
/// declining to guess rather than claiming they are free, and the list below is
/// what makes the remainder countable instead of invisible.
///
/// The patience is handed in because it is a knob: an operator on a slow disk
/// runs with a longer one, and a length stated from a constant would be the
/// wrong length for exactly the run that needed the knob.
#[must_use]
pub const fn of(command: &Command, patience: Duration) -> Option<Disturbance> {
    let Some(situation) = situation(command) else {
        return None;
    };
    Some(situation.takes(patience))
}

/// Which situation this command puts the stack in, or none where this does not
/// yet say.
///
/// Split from [`of`] so that what a command *is* and how long that *takes* are
/// answered in different places: the first is a routing decision this file owns,
/// the second is a length the run is held to. A payload listing every situation
/// reads the second half without going near the first.
#[must_use]
const fn situation(command: &Command) -> Option<Situation> {
    match command {
        // Apart from one another because they are apart in [`Command`], and a
        // day where a service start is held to a different clock from a form
        // start is a day this reads as two lines rather than being rewritten.
        // A boot joins the two because it runs the same start in the middle by
        // calling it, and is held to the same clock. Nobody is watching one —
        // that is the whole of why it reports to a store rather than to a person
        // — but the length it was prepared to wait is exactly what somebody reads
        // back afterwards to understand why a four-in-the-morning start gave up
        // when it did.
        Command::Up { .. } | Command::Start { .. } | Command::AtBoot => Some(Situation::Starting),
        Command::Restart { .. } => Some(Situation::Restarting),
        Command::Switch { .. } => Some(Situation::Switching),
        Command::Down {
            wait: Waiting::ForTheDownloads,
            ..
        } => Some(Situation::StoppingAfterDownloads),
        Command::Down { .. } | Command::Halt { .. } => Some(Situation::Stopping),
        // Everything else, listed rather than left to a wildcard. A command
        // added here would otherwise answer *disturbs nothing* by default, and
        // a machine taken away in silence is the failure this exists to prevent —
        // so a new one stops the build until somebody has decided.
        Command::Version
        | Command::Catalogue
        | Command::Forms
        | Command::Preview { .. }
        | Command::Pull { .. }
        | Command::ConfigGet { .. }
        | Command::ConfigSet(..)
        | Command::ConfigShow
        | Command::Ps { .. }
        | Command::Doctor { .. }
        | Command::Repair { .. }
        | Command::Undo { .. }
        | Command::Quality(..)
        | Command::Alerts(..)
        | Command::History
        | Command::Migrate(..)
        | Command::QualityUpgrade { .. }
        | Command::QualityMusic { .. }
        | Command::Trace { .. }
        | Command::Household { .. }
        | Command::Held { .. }
        | Command::Allowing(..)
        | Command::Deciding(..)
        | Command::Expiring(..)
        | Command::Stuck
        | Command::FrontDoor
        | Command::Explain { .. }
        | Command::Glossary
        | Command::Clients
        | Command::Invite { .. }
        | Command::Reissue { .. }
        | Command::Remove { .. }
        | Command::Outbound
        | Command::Provenance
        | Command::Credentials(..)
        | Command::Stored
        | Command::Forget { .. }
        | Command::SelfUpdate { .. }
        | Command::Uninstall(..)
        | Command::Space { .. }
        | Command::StopSeeding { .. }
        | Command::Bandwidth(..)
        | Command::Watch { .. }
        | Command::Hosting(..)
        | Command::Walkthrough { .. }
        | Command::Seed
        | Command::Adopt
        | Command::Reset { .. }
        | Command::Update(..)
        | Command::Backup { .. }
        | Command::Support { .. }
        | Command::Archives
        | Command::Restore { .. }
        | Command::Setup(..) => None,
    }
}

/// Say what this is about to take away, before it takes it.
///
/// Said through the narrator rather than returned, because it belongs to the
/// moment before the run rather than to the report after it — a length an
/// operator reads once the thing has already stopped is a length they had no
/// use for.
///
/// A rehearsal says it too. What somebody rehearsing wants to know is what the
/// real run would cost, and a rehearsal silent about it would leave the one
/// question the flag exists to answer unanswered.
pub async fn said(command: &Command, ctx: &Ctx) {
    let Some(disturbance) = of(command, ctx.patience) else {
        return;
    };

    ctx.narrator.say(&sentence(disturbance)).await;
}

/// The one place a disturbance is put into words.
///
/// The bounded arm reaches for the same phrasing a disturbing check uses, so the
/// two kinds of length an operator meets read the same way and neither can drift
/// into a number typed into a sentence.
fn sentence(disturbance: Disturbance) -> String {
    match disturbance {
        Disturbance::Bounded(bound) => format!(
            "this takes services away {}",
            crate::doctor::disturbing_for(bound)
        ),
        Disturbance::Until(Awaiting::Downloads) => {
            "this does not finish until everything still coming down has arrived".to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A patience unlike any default, so a length read from it cannot be a
    /// constant that happens to match.
    const WAITED: Duration = Duration::from_secs(41);

    #[test]
    fn starting_is_bounded_by_the_patience_the_run_was_given() {
        let command = Command::Up {
            forms: vec!["media".to_owned()],
        };

        assert_eq!(
            of(&command, WAITED),
            Some(Disturbance::Bounded(WAITED)),
            "a start is held to the same wait that decides when it gives up"
        );
    }

    #[test]
    fn every_verb_that_starts_something_is_bounded_the_same_way() {
        let starting = [
            Command::Up { forms: Vec::new() },
            Command::Start {
                forms: Vec::new(),
                services: vec!["sonarr".to_owned()],
            },
            Command::Restart {
                forms: Vec::new(),
                services: Vec::new(),
            },
            Command::Switch { forms: Vec::new() },
        ];

        for command in starting {
            assert_eq!(
                of(&command, WAITED),
                Some(Disturbance::Bounded(WAITED)),
                "{command:?} waits for services to settle and is bounded by it"
            );
        }
    }

    #[test]
    fn a_teardown_that_waits_for_downloads_has_nothing_bounding_it() {
        let command = Command::Down {
            forms: Vec::new(),
            wait: Waiting::ForTheDownloads,
        };

        assert_eq!(
            of(&command, WAITED),
            Some(Disturbance::Until(Awaiting::Downloads)),
            "how long a torrent takes belongs to whoever is seeding it"
        );
    }

    #[test]
    fn a_teardown_that_does_not_wait_is_bounded_by_the_engines_grace() {
        let command = Command::Down {
            forms: Vec::new(),
            wait: Waiting::Never,
        };

        assert_eq!(
            of(&command, WAITED),
            Some(Disturbance::Bounded(GRACE)),
            "a stop is held to what the engine gives a service, not to the settle wait"
        );
    }

    #[test]
    fn stopping_named_services_is_the_same_stop() {
        let command = Command::Halt {
            forms: Vec::new(),
            services: vec!["sonarr".to_owned()],
        };

        assert_eq!(of(&command, WAITED), Some(Disturbance::Bounded(GRACE)));
    }

    #[test]
    fn reading_takes_nothing_away_and_says_nothing() {
        let reading = [Command::Version, Command::Forms, Command::History];

        for command in reading {
            assert_eq!(
                of(&command, WAITED),
                None,
                "{command:?} disturbs nobody, and a line saying so teaches an operator to skip it"
            );
        }
    }

    #[test]
    fn a_bounded_length_is_said_in_the_one_place_a_length_becomes_words() {
        let said = sentence(Disturbance::Bounded(WAITED));

        assert!(
            said.contains(&crate::doctor::disturbing_for(WAITED)),
            "a length typed into a sentence goes stale the day the bound moves: {said}"
        );
    }

    #[test]
    fn an_unbounded_operation_says_what_it_is_waiting_for() {
        let said = sentence(Disturbance::Until(Awaiting::Downloads));

        assert!(
            said.contains("coming down"),
            "open-ended is half an answer without what it is open-ended on: {said}"
        );
        assert!(
            !said.contains("seconds"),
            "a wait with no bound must not be given one in words: {said}"
        );
    }

    /// The payload states every situation there is, and nothing else.
    ///
    /// This is the join that keeps the two halves from drifting apart. A new
    /// situation fails to compile in [`Situation::called`], which sends whoever
    /// added it to [`Situation::EVERY`], which lands here — and this stays red
    /// until the payload has grown a field for it. Without the join, a situation
    /// could be added, routed, and held to a clock while every surface went on
    /// publishing the four it knew about, which reads to an operator as though
    /// the new verb costs nothing.
    ///
    /// Both directions, because a field nothing routes to is the same silence
    /// wearing the opposite hat: a published length no command is ever held to.
    #[test]
    fn the_payload_states_every_situation_there_is_and_nothing_else() {
        let published =
            serde_json::to_value(crate::model::Disturbances::all(WAITED)).unwrap_or_default();
        let fields = published.as_object().cloned().unwrap_or_default();

        assert!(
            !fields.is_empty(),
            "the payload serialised to nothing, so the rest of this rule read nothing"
        );

        for situation in Situation::EVERY.iter().copied() {
            assert!(
                fields.contains_key(situation.called()),
                "{situation:?} is a situation the stack can be put in and no surface \
                 can read what it costs"
            );
        }

        // Both sides are named before the assertion rather than inside its message.
        // A message is rendered only where the assertion fails, so a run that passes
        // never enters it — and the coverage gate counts a rendering no run reaches
        // against every covered line beside it.
        let published: Vec<&String> = fields.keys().collect();
        let known: Vec<&str> = Situation::EVERY
            .iter()
            .copied()
            .map(Situation::called)
            .collect();
        assert_eq!(
            published.len(),
            known.len(),
            "the payload publishes a length nothing is ever held to: {published:?} \
             against {known:?}"
        );
    }

    /// What a surface reads off the payload is what the command it then runs is
    /// held to.
    ///
    /// The whole point of publishing the lengths is that a surface states one and
    /// then acts; if the two were arrived at separately, the day they diverged
    /// would be a day an operator was told a number nothing honoured — and
    /// nothing on either side would ever compare them, so it would never be
    /// found. This is that comparison, made once, for every command that has a
    /// length at all.
    #[test]
    fn the_payload_and_the_command_agree_on_every_length() {
        let forms = || vec!["media".to_owned()];
        let services = || vec!["sonarr".to_owned()];
        let payload = crate::model::Disturbances::all(WAITED);

        let pairs = [
            (Command::Up { forms: forms() }, payload.starting),
            (
                Command::Start {
                    forms: forms(),
                    services: services(),
                },
                payload.starting,
            ),
            (
                Command::Down {
                    forms: forms(),
                    wait: Waiting::Never,
                },
                payload.stopping,
            ),
            (
                Command::Halt {
                    forms: forms(),
                    services: services(),
                },
                payload.stopping,
            ),
            (
                Command::Down {
                    forms: forms(),
                    wait: Waiting::ForTheDownloads,
                },
                payload.stopping_after_downloads,
            ),
            (
                Command::Restart {
                    forms: forms(),
                    services: services(),
                },
                payload.restarting,
            ),
            (Command::Switch { forms: forms() }, payload.switching),
        ];

        for (command, published) in pairs {
            // Compared as options rather than unwrapped, so that a command which
            // stops having a length at all fails here saying which one, instead of
            // ending the run at a line that names nothing.
            assert_eq!(
                of(&command, WAITED).map(crate::model::TakesAway::from),
                Some(published),
                "{command:?} is held to one length and the payload publishes another, \
                 so a surface states a number no run honours"
            );
        }
    }

    /// Addressing a form and addressing the services inside it take the same
    /// length away.
    ///
    /// This is what lets one published field answer for two commands. It is
    /// asserted rather than assumed, because the day a service stop is held to a
    /// different clock from a teardown, the payload has to grow a field — and the
    /// failure that says so has to arrive here, not in an operator being told the
    /// wrong number.
    #[test]
    fn a_form_and_the_services_inside_it_are_held_to_the_same_clock() {
        let forms = vec!["media".to_owned()];
        let services = vec!["sonarr".to_owned()];

        let same = [
            (
                Command::Up {
                    forms: forms.clone(),
                },
                Command::Start {
                    forms: forms.clone(),
                    services: services.clone(),
                },
            ),
            (
                Command::Down {
                    forms: forms.clone(),
                    wait: Waiting::Never,
                },
                Command::Halt {
                    forms: forms.clone(),
                    services,
                },
            ),
        ];

        for (whole, named) in same {
            assert_eq!(
                of(&whole, WAITED),
                of(&named, WAITED),
                "{whole:?} and {named:?} no longer cost the same, so one published \
                 field can no longer answer for both"
            );
        }
    }

    /// The published lengths are the knob's, not a constant's.
    ///
    /// An operator on a slow disk runs with a longer patience, and a payload that
    /// answered from a constant would state the length somebody else's stack was
    /// held to — plausibly, and wrong in exactly the case the knob exists for.
    #[test]
    fn a_longer_patience_is_a_longer_published_start() {
        let brief = crate::model::Disturbances::all(Duration::from_secs(30));
        let patient = crate::model::Disturbances::all(Duration::from_secs(600));

        assert_ne!(
            brief.starting, patient.starting,
            "the published start ignores the knob it is supposed to be reading"
        );
        assert_eq!(
            brief.stopping, patient.stopping,
            "stopping is held to the engine's grace, which the start knob does not move"
        );
    }
}
