//! What a reversal that cannot go ahead says: nowhere to look for the record, no run
//! by the stamp named, more than one, and a change that cannot be put back.

use crate::error::codes::undo::{CANNOT_SUCCEED, MORE_THAN_ONE_RUN, NOWHERE_TO_LOOK, NO_SUCH_RUN};
use crate::error::{Problem, Remedy, Severity, State};

pub(super) fn nowhere_to_look() -> Problem {
    Problem::new(
        NOWHERE_TO_LOOK,
        Severity::Error,
        "This run has nowhere it knows to look for what was changed",
        "What lemonfiber changed is recorded in its own directory, and this machine \
         would not say where that is. Nothing was put back.",
        Remedy::new("Set a home directory for this user and run it again"),
    )
    .in_state(State::Guided)
}

pub(super) fn no_such_run(at: &str) -> Problem {
    Problem::new(
        NO_SUCH_RUN,
        Severity::Error,
        format!("Nothing was changed at {at}"),
        format!(
            "No run in the record carries the stamp {at}. It may have fallen outside the \
             horizon the record keeps, or the stamp may be mistyped. Nothing was put back."
        ),
        Remedy::new("Run `lemonfiber history` and take the stamp from the entry you want"),
    )
    .in_state(State::Actionable)
}

pub(super) fn more_than_one(at: &str, operations: &[&str]) -> Problem {
    Problem::new(
        MORE_THAN_ONE_RUN,
        Severity::Error,
        format!("More than one run is stamped {at}"),
        format!(
            "{at} names {}, and putting back the wrong one is not something to guess at. \
             Nothing was put back.",
            operations.join(" and ")
        ),
        Remedy::new("Ask for one of them by name once the surfaces carry it"),
    )
    .in_state(State::Actionable)
}

pub(super) fn cannot_succeed(at: &str, target: &str, why: &str) -> Problem {
    Problem::new(
        CANNOT_SUCCEED,
        Severity::Error,
        format!("The run stamped {at} cannot be put back"),
        format!(
            "One of its changes, against {target}, cannot be reversed: {why}. A run goes \
             back whole or not at all, so nothing was put back."
        ),
        Remedy::new("Deal with that change first, or restore from a backup"),
    )
    .in_state(State::Actionable)
}
