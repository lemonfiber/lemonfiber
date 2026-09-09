//! What an operator may do about a setup that is already here.
//!
//! Four modes, and the posture between them is the whole of what this module decides.
//! **Adopt** is the default: keep what is running and put lemonfiber in front of it.
//! **Replace** is offered and never preselected — it is the one that stops somebody's
//! working stack, and a mode that destructive has to be reached for rather than
//! arrived at.
//!
//! None of them is carried out here. This says what each would come to, which is what
//! an operator reads before choosing, and choosing is a separate act.

use std::collections::BTreeSet;

use crate::model::{ModeReport, MovedReport};

use super::Ours;

/// What to do about a setup already on the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Keep the existing services and put lemonfiber in front of them.
    #[default]
    Adopt,
    /// Stand up lemonfiber's own stack and copy what can be copied across.
    Import,
    /// Stand up a clean stack and stop the old one, deleting nothing.
    Replace,
    /// Run on alternative ports alongside the existing setup.
    Beside,
}

impl Mode {
    /// The one word this mode is named by where a name has to be a single token — a
    /// web action, a subcommand — as against the words a person reads.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Adopt => "adopt",
            Self::Import => "import",
            Self::Replace => "replace",
            Self::Beside => "beside",
        }
    }

    /// The word an operator types for this mode.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Adopt => "adopt",
            Self::Import => "import",
            Self::Replace => "replace",
            Self::Beside => "side-by-side",
        }
    }

    /// Whether this mode is offered already chosen.
    ///
    /// Only adopt is. Replace stops a working stack, and side-by-side and import both
    /// stand up a second copy of everything — none of which is a thing to find already
    /// ticked.
    #[must_use]
    pub fn preselected(self) -> bool {
        self == Self::default()
    }

    /// Whether carrying this mode out stops or alters what is already running.
    #[must_use]
    pub const fn disturbs_what_is_running(self) -> bool {
        matches!(self, Self::Replace)
    }

    /// What choosing this mode would come to, in the operator's terms.
    #[must_use]
    pub const fn what(self) -> &'static str {
        match self {
            Self::Adopt => {
                "lemonfiber manages the containers already here and reads their configuration. \
                 Nothing is recreated and nothing is moved."
            }
            Self::Import => {
                "lemonfiber stands up its own stack and copies indexers, root folders and \
                 monitored items across through each service's own API. The existing stack is \
                 left running."
            }
            Self::Replace => {
                "lemonfiber stands up a clean stack and stops the old one. Nothing of it is \
                 deleted, so it can be started again."
            }
            Self::Beside => {
                "lemonfiber runs on ports nothing else is using, alongside the setup already \
                 here, so it can be tried without committing to it."
            }
        }
    }
}

/// Every mode, in the order they are offered.
///
/// Adopt first because it is the default and the least destructive; replace last
/// because it is the one that stops something working.
pub const EVERY: [Mode; 4] = [Mode::Adopt, Mode::Import, Mode::Beside, Mode::Replace];

/// What each mode would come to, for an operator deciding between them.
#[must_use]
pub fn offered() -> Vec<ModeReport> {
    EVERY
        .iter()
        .map(|mode| ModeReport {
            mode: mode.word().to_owned(),
            preselected: mode.preselected(),
            what: mode.what().to_owned(),
            disturbs: mode.disturbs_what_is_running(),
        })
        .collect()
}

/// Where lemonfiber's services would listen to run beside what is already here.
///
/// The lowest free port above the one it would ordinarily take, so an operator
/// evaluating a second copy can still guess where each service is. A port already
/// answering, one another lemonfiber service wants, and one already handed out here are
/// all skipped.
///
/// A service whose port cannot be moved anywhere is left out rather than given a
/// number that will not work.
#[must_use]
pub fn beside(ours: &[Ours], taken: &BTreeSet<u16>) -> Vec<MovedReport> {
    let wanted: BTreeSet<u16> = ours.iter().filter_map(|one| one.port).collect();
    let mut handed: BTreeSet<u16> = BTreeSet::new();
    let mut moved: Vec<MovedReport> = Vec::new();

    let mut order: Vec<(&str, u16)> = ours
        .iter()
        .filter_map(|one| one.port.map(|port| (one.service.as_str(), port)))
        .collect();
    order.sort_by(|one, two| one.1.cmp(&two.1).then(one.0.cmp(two.0)));

    for (service, port) in order {
        let Some(free) = (port..u16::MAX)
            .skip(1)
            .find(|free| !taken.contains(free) && !wanted.contains(free) && !handed.contains(free))
        else {
            continue;
        };
        handed.insert(free);
        moved.push(MovedReport {
            service: service.to_owned(),
            from: port,
            to: free,
        });
    }
    moved
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{beside, offered, Mode, EVERY};
    use crate::migration::tests::ours;
    use crate::migration::Ours;

    fn wanting(services: &[(&str, u16)]) -> Vec<Ours> {
        services
            .iter()
            .map(|(service, port)| ours(service, service, "1", Some(*port)))
            .collect()
    }

    #[test]
    fn adopting_is_what_an_operator_finds_already_chosen() {
        assert_eq!(Mode::default(), Mode::Adopt);
        assert!(Mode::Adopt.preselected());
    }

    #[test]
    fn replacing_a_working_stack_is_never_found_already_chosen() {
        assert!(!Mode::Replace.preselected());
        let preselected: Vec<&str> = EVERY
            .iter()
            .filter(|mode| mode.preselected())
            .map(|mode| mode.word())
            .collect();
        assert_eq!(preselected, vec!["adopt"], "exactly one, and it is adopt");
    }

    #[test]
    fn replacing_is_the_only_mode_that_stops_what_is_running() {
        let disturbing: Vec<&str> = EVERY
            .iter()
            .filter(|mode| mode.disturbs_what_is_running())
            .map(|mode| mode.word())
            .collect();
        assert_eq!(disturbing, vec!["replace"]);
    }

    #[test]
    fn every_mode_is_offered_and_says_what_it_would_come_to() {
        let read = offered();
        assert_eq!(read.len(), EVERY.len());
        assert!(read.iter().all(|mode| !mode.what.is_empty()), "{read:?}");
    }

    #[test]
    fn the_least_destructive_mode_is_offered_first_and_the_most_last() {
        let order: Vec<String> = offered().into_iter().map(|mode| mode.mode).collect();
        assert_eq!(order.first().map(String::as_str), Some("adopt"));
        assert_eq!(order.last().map(String::as_str), Some("replace"));
    }

    #[test]
    fn running_beside_takes_the_next_port_up_so_it_can_still_be_guessed() {
        let moved = beside(&wanting(&[("sonarr", 8989)]), &BTreeSet::new());
        let to = moved.first().map(|one| one.to);
        assert_eq!(to, Some(8990), "{moved:?}");
    }

    #[test]
    fn a_port_something_else_answers_on_is_stepped_over() {
        let taken: BTreeSet<u16> = [8990, 8991].into_iter().collect();
        let moved = beside(&wanting(&[("sonarr", 8989)]), &taken);
        let to = moved.first().map(|one| one.to);
        assert_eq!(to, Some(8992), "{moved:?}");
    }

    #[test]
    fn a_port_another_of_our_own_services_wants_is_stepped_over() {
        let moved = beside(
            &wanting(&[("sonarr", 8989), ("radarr", 8990)]),
            &BTreeSet::new(),
        );
        let sonarr = moved
            .iter()
            .find(|one| one.service == "sonarr")
            .map(|one| one.to);
        assert_eq!(sonarr, Some(8991), "{moved:?}");
    }

    #[test]
    fn no_two_services_are_handed_the_same_port() {
        let moved = beside(
            &wanting(&[("sonarr", 8989), ("radarr", 8989), ("lidarr", 8989)]),
            &BTreeSet::new(),
        );
        let given: BTreeSet<u16> = moved.iter().map(|one| one.to).collect();
        assert_eq!(given.len(), moved.len(), "{moved:?}");
    }

    #[test]
    fn a_service_with_nowhere_left_to_go_is_left_out_rather_than_given_a_bad_port() {
        let moved = beside(&wanting(&[("sonarr", u16::MAX - 1)]), &BTreeSet::new());
        assert!(moved.is_empty(), "{moved:?}");
    }

    #[test]
    fn a_service_with_no_listener_is_not_given_a_port_to_move_to() {
        let quiet = [ours("recyclarr", "recyclarr", "1", None)];
        assert!(beside(&quiet, &BTreeSet::new()).is_empty());
    }

    #[test]
    fn what_is_moved_reads_in_a_settled_order() {
        let moved = beside(
            &wanting(&[("radarr", 7878), ("sonarr", 8989)]),
            &BTreeSet::new(),
        );
        let order: Vec<u16> = moved.iter().map(|one| one.from).collect();
        assert_eq!(order, vec![7878, 8989]);
    }
}
