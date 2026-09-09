//! How often the release list is asked, and when to stop asking it.
//!
//! Two rules, and both exist to keep a check nobody asked for from becoming a thing
//! the operator notices. The first is that asking is rare: a version that came out
//! this morning is no more useful to know about now than in an hour, so the answer
//! is remembered and reused, and running the same read twice reaches the network
//! once. The second is that failing gets quieter rather than louder — a machine with
//! no route out would otherwise reach for the network on every run for ever, which
//! is the shape a thing has when it is about to be described as spyware.
//!
//! Failing enough times in a row stops it entirely. That is deliberate and it is the
//! only state here with no way back on its own: a laptop that has been off the
//! network for a week should not still be trying, and the operator who wants an
//! answer anyway has a way to say so.

use serde::{Deserialize, Serialize};

/// How long to leave between asking, once.
const APART: u64 = 60 * 60 * 24;

/// How many failures in a row before it stops being attempted at all.
const GIVEN_UP: u32 = 5;

/// The longest the wait is allowed to double to, in failures.
///
/// Beyond this the wait stops growing, which matters only because the arithmetic
/// would otherwise leave the realm of numbers a machine holds. Reached before
/// [`GIVEN_UP`] would stop it anyway, so this is a bound rather than a policy.
const DOUBLINGS: u32 = 4;

/// What the last checks came to, kept between runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Noticed {
    /// When the release list was last asked, in seconds since the epoch.
    asked: Option<u64>,
    /// The newest version the last answered check read.
    offered: Option<String>,
    /// How many checks in a row have not been answered.
    quiet: u32,
}

impl Noticed {
    /// Whether the release list should be asked now.
    ///
    /// A machine that has never asked always should. Otherwise the wait since the
    /// last attempt has to have passed, and enough failures in a row means it never
    /// should again — which is what "stops being attempted" comes to, and is why
    /// this is the one answer here that does not come back on its own.
    #[must_use]
    pub fn due(&self, now: u64) -> bool {
        if self.quiet >= GIVEN_UP {
            return false;
        }
        let Some(asked) = self.asked else {
            return true;
        };
        now.saturating_sub(asked) >= self.wait()
    }

    /// How long to leave before asking again, given how the last few went.
    ///
    /// Doubling per failure, so a machine with no route out is asking once a day,
    /// then every other day, then every fourth. A check that answers puts it back to
    /// once a day, because the reason for the long wait has gone.
    fn wait(&self) -> u64 {
        APART << self.quiet.min(DOUBLINGS)
    }

    /// Record a check that was answered, and what it read.
    pub fn answered(&mut self, now: u64, offered: Option<String>) {
        self.asked = Some(now);
        self.quiet = 0;
        if offered.is_some() {
            self.offered = offered;
        }
    }

    /// Record a check that nothing answered.
    pub fn silent(&mut self, now: u64) {
        self.asked = Some(now);
        self.quiet = self.quiet.saturating_add(1);
    }

    /// The newest version the last answered check read, where one has.
    #[must_use]
    pub fn remembered(&self) -> Option<&str> {
        self.offered.as_deref()
    }

    /// Whether asking has been given up on, which is worth saying rather than
    /// leaving as an answer that never changes.
    #[must_use]
    pub const fn given_up(&self) -> bool {
        self.quiet >= GIVEN_UP
    }
}

/// Why availability could not be told, where it could not.
///
/// Four reasons, and they differ in what an operator would do about each: one is a
/// setting they chose, one is a machine that has stopped asking, one is an address
/// that said nothing, and one is a check that is simply not due. None of them is a
/// fault, and none of them stops anything — which is why this is a reason rather than
/// a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Silence {
    /// The operator switched the check off.
    Refused,
    /// Enough checks in a row went unanswered that it is no longer attempted.
    GivenUp,
    /// The address was asked and said nothing this could read a version out of.
    Unanswered,
    /// Nothing has been read yet and it is not time to ask again.
    NotYet,
}

impl Silence {
    /// What to say about it, and what to do about it where there is anything.
    #[must_use]
    pub const fn why(self) -> &'static str {
        match self {
            Self::Refused => {
                "The release list is not asked, because this machine's settings say so. \
                 Nothing else changes: every other thing lemonfiber does works exactly as \
                 well without knowing whether a version came out."
            }
            Self::GivenUp => {
                "Enough checks in a row went unanswered that this machine has stopped \
                 asking. That is deliberate — a laptop with no route out should not reach \
                 for the network on every run for ever — and it starts again once a check \
                 is answered."
            }
            Self::Unanswered => {
                "The release list did not answer, or answered with nothing a version could \
                 be read out of. Nothing waits on it, and the next attempt is further off \
                 than the last one was."
            }
            Self::NotYet => {
                "The release list has not been asked yet today, and nothing has been read \
                 from it before. A version that came out this morning is no more useful to \
                 know about now than in an hour."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Noticed, Silence, APART, GIVEN_UP};

    /// A record that answered at this moment and read this version.
    fn answered(at: u64, offered: Option<&str>) -> Noticed {
        let mut noticed = Noticed::default();
        noticed.answered(at, offered.map(str::to_owned));
        noticed
    }

    /// A record that has failed this many times in a row, the last at this moment.
    fn silent(times: u32, at: u64) -> Noticed {
        let mut noticed = Noticed::default();
        for _ in 0..times {
            noticed.silent(at);
        }
        noticed
    }

    #[test]
    fn a_machine_that_has_never_asked_asks() {
        assert!(Noticed::default().due(0));
        assert_eq!(Noticed::default().remembered(), None);
        assert!(!Noticed::default().given_up());
    }

    #[test]
    fn asking_twice_in_a_row_reaches_the_network_once() {
        let noticed = answered(1_000, Some("0.13.0"));
        assert!(!noticed.due(1_000));
        assert!(!noticed.due(1_000 + APART - 1));
        assert!(noticed.due(1_000 + APART));
    }

    #[test]
    fn what_the_last_answer_read_is_remembered_so_the_next_run_need_not_ask() {
        assert_eq!(answered(1_000, Some("0.13.0")).remembered(), Some("0.13.0"));
    }

    /// A check that reached the address and could make nothing of the answer still
    /// counts as answered — nothing failed — and must not throw away what an earlier
    /// one had read.
    #[test]
    fn an_answer_holding_no_version_leaves_the_one_already_known_alone() {
        let mut noticed = answered(1_000, Some("0.13.0"));
        noticed.answered(2_000, None);
        assert_eq!(noticed.remembered(), Some("0.13.0"));
        assert!(!noticed.given_up());
    }

    #[test]
    fn each_failure_leaves_a_longer_wait_than_the_one_before() {
        let mut last = 0;
        for failures in 1..GIVEN_UP {
            let noticed = silent(failures, 0);
            let waited = (1..APART * 64)
                .find(|now| noticed.due(*now))
                .unwrap_or_default();
            assert!(
                waited > last,
                "{failures} failures waited {waited}, no longer than {last}"
            );
            last = waited;
        }
    }

    #[test]
    fn failing_enough_times_in_a_row_stops_it_being_attempted_at_all() {
        let noticed = silent(GIVEN_UP, 0);
        assert!(noticed.given_up());
        assert!(!noticed.due(0));
        assert!(!noticed.due(APART * 1_000));
    }

    #[test]
    fn a_check_that_answers_puts_the_wait_back_to_where_it_started() {
        let mut noticed = silent(GIVEN_UP - 1, 0);
        assert!(!noticed.due(APART));
        noticed.answered(APART * 100, Some("0.13.0".to_owned()));
        assert!(!noticed.due(APART * 100));
        assert!(noticed.due(APART * 101));
    }

    /// A clock that went backwards between two runs is a laptop that suspended
    /// across a time-zone change, and it must not turn into a wait of decades.
    #[test]
    fn a_clock_that_went_backwards_is_read_as_no_time_having_passed() {
        assert!(!answered(1_000_000, Some("0.13.0")).due(1));
    }

    /// Each reason says which it is and what, if anything, to do about it. A single
    /// "could not tell" would leave an operator who turned the check off looking for a
    /// network fault.
    #[test]
    fn each_reason_for_not_knowing_is_a_different_thing_to_do_about_it() {
        let reasons = [
            Silence::Refused,
            Silence::GivenUp,
            Silence::Unanswered,
            Silence::NotYet,
        ];
        let said: Vec<&str> = reasons.iter().map(|reason| reason.why()).collect();
        for one in &said {
            assert!(one.split_whitespace().count() >= 20, "{one}");
        }
        let mut distinct = said.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), said.len());
        assert!(Silence::Refused.why().contains("settings say so"));
        assert!(Silence::GivenUp.why().contains("stopped asking"));
    }

    #[test]
    fn what_is_remembered_survives_being_written_down_and_read_back() {
        let noticed = answered(1_000, Some("0.13.0"));
        let written = serde_json::to_string(&noticed).unwrap_or_default();
        let read: Noticed = serde_json::from_str(&written).unwrap_or_default();
        assert_eq!(read, noticed);
    }
}
