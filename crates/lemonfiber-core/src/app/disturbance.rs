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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Awaiting {
    /// Everything still coming down has finished arriving.
    Downloads,
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
    match command {
        // Starting is bounded by how long the run waits for services to settle,
        // which is the same knob that decides when it gives up. A start that
        // has to fetch an image spends that time before this one begins, and it
        // is narrated as it happens rather than promised in advance.
        Command::Up { .. }
        | Command::Start { .. }
        | Command::Restart { .. }
        | Command::Switch { .. } => Some(Disturbance::Bounded(patience)),
        // A teardown told to let the downloads finish is the one operation here
        // with nothing to bound it: what it waits for is a torrent at ninety-four
        // per cent, and how long that takes belongs to whoever is seeding it.
        Command::Down {
            wait: Waiting::ForTheDownloads,
            ..
        } => Some(Disturbance::Until(Awaiting::Downloads)),
        Command::Down { .. } | Command::Halt { .. } => Some(Disturbance::Bounded(GRACE)),
        // Everything else, listed rather than left to a wildcard. A command
        // added here would otherwise answer *disturbs nothing* by default, and
        // a machine taken away in silence is the failure this exists to prevent —
        // so a new one stops the build until somebody has decided.
        Command::Version
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
}
