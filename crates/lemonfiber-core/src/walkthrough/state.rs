//! Where a walkthrough stands, as the one word a caller can act on.
//!
//! Kept apart from [`Step`](super::Step) because they answer different questions. A step
//! is a place on the walk; a state is what has become of the walk itself — offered,
//! declined, running at some step, finished, stopped, or left. Two of the states have no
//! step at all, which is why they cannot be the same type.

use serde::{Deserialize, Serialize};

use super::Step;

/// What has become of a walkthrough.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Presented at the end of setup, not yet answered.
    #[default]
    Offered,
    /// Declined. Available later, and carrying no penalty — a stack that was set up is
    /// set up whether or not anyone watched it fetch something.
    Skipped,
    /// Looking for releases.
    Searching,
    /// Sending a release to the download client.
    Grabbing,
    /// The download is running.
    Downloading,
    /// Moving the finished download into the library.
    Importing,
    /// It is in the library and playable.
    Complete,
    /// Stopped at a named step, with a diagnosis.
    Failed,
    /// The operator left part-way. Whatever was in flight stays in flight.
    Abandoned,
}

impl State {
    /// The state a walk is in while it is working on `step`.
    ///
    /// The last step is not a state of its own while running — reaching it *is* being
    /// complete — and the first is not either: something still being chosen has not
    /// started, so it reads as the offer it came from.
    #[must_use]
    pub const fn of_step(step: Step) -> Self {
        match step {
            Step::Choosing => Self::Offered,
            Step::Searching => Self::Searching,
            Step::Grabbing => Self::Grabbing,
            Step::Downloading => Self::Downloading,
            // Telling the library to look is the tail of the import as far as the
            // operator is concerned: nothing new is being fetched, it is being filed.
            Step::Importing | Step::Scanning => Self::Importing,
            Step::Available => Self::Complete,
        }
    }

    /// Whether the walk is still going. A caller polling one asks this rather than
    /// listing the states it has to keep waiting through.
    #[must_use]
    pub const fn is_running(self) -> bool {
        matches!(
            self,
            Self::Searching | Self::Grabbing | Self::Downloading | Self::Importing
        )
    }

    /// Whether anything is still owed to the operator. A walk they declined, left, or
    /// saw fail is finished with them — only a running one has more to say.
    #[must_use]
    pub const fn is_settled(self) -> bool {
        !self.is_running() && !matches!(self, Self::Offered)
    }

    /// Whether the stack is worse off for this ending.
    ///
    /// Only a failure is: declining costs nothing by design, and leaving part-way leaves
    /// a download running rather than a mess — which is the point of saying so.
    #[must_use]
    pub const fn is_a_problem(self) -> bool {
        matches!(self, Self::Failed)
    }

    /// This state as the word a machine-readable consumer reads, matching the states the
    /// specification names.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Offered => "offered",
            Self::Skipped => "skipped",
            Self::Searching => "searching",
            Self::Grabbing => "grabbing",
            Self::Downloading => "downloading",
            Self::Importing => "importing",
            Self::Complete => "complete",
            Self::Failed => "failed",
            Self::Abandoned => "abandoned",
        }
    }

    /// Every state, so a caller can prove it handles all of them.
    #[must_use]
    pub const fn all() -> [Self; 9] {
        [
            Self::Offered,
            Self::Skipped,
            Self::Searching,
            Self::Grabbing,
            Self::Downloading,
            Self::Importing,
            Self::Complete,
            Self::Failed,
            Self::Abandoned,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use super::Step;

    #[test]
    fn every_step_of_the_walk_has_a_state_to_report_it_by() {
        for step in Step::all() {
            let state = State::of_step(step);
            // A step that is being worked on is either running or one of the two ends.
            assert!(
                state.is_running() || matches!(state, State::Offered | State::Complete),
                "{step:?} reported as {state:?}"
            );
        }
    }

    #[test]
    fn reaching_the_last_step_is_being_complete() {
        assert_eq!(State::of_step(Step::Available), State::Complete);
        assert_eq!(State::of_step(Step::Choosing), State::Offered);
        // Filing what arrived is still importing as far as the operator is concerned.
        assert_eq!(State::of_step(Step::Scanning), State::Importing);
    }

    #[test]
    fn only_a_failure_is_a_problem() {
        let problems: Vec<State> = State::all()
            .into_iter()
            .filter(|state| state.is_a_problem())
            .collect();
        assert_eq!(
            problems,
            vec![State::Failed],
            "declining and leaving cost nothing by design"
        );
    }

    #[test]
    fn a_walk_is_settled_once_it_stops_owing_the_operator_anything() {
        let unsettled: Vec<State> = State::all()
            .into_iter()
            .filter(|state| !state.is_settled())
            .collect();
        assert_eq!(
            unsettled,
            vec![
                State::Offered,
                State::Searching,
                State::Grabbing,
                State::Downloading,
                State::Importing
            ]
        );
    }

    #[test]
    fn every_state_has_the_word_the_specification_names_it_by() {
        let words: Vec<&str> = State::all().into_iter().map(State::word).collect();
        assert_eq!(
            words,
            vec![
                "offered",
                "skipped",
                "searching",
                "grabbing",
                "downloading",
                "importing",
                "complete",
                "failed",
                "abandoned"
            ]
        );
    }
}
