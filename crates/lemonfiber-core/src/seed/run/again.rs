//! A second try at the connections a pass found not answering.

use crate::seed::{Report, State, Wiring, NOT_ANSWERING};

/// Whether a pass skipped `wiring` because the service it reaches was not answering.
pub(super) fn unanswered(wiring: &Wiring) -> bool {
    matches!(&wiring.state, State::Skipped { reason } if reason == NOT_ANSWERING)
}

/// What `first` found, each connection it found not answering as `later` found it.
pub(super) fn retried(first: Report, later: &Report) -> Report {
    let wirings = first
        .wirings
        .into_iter()
        .map(|wiring| {
            if !unanswered(&wiring) {
                return wiring;
            }
            later
                .wirings
                .iter()
                .find(|again| again.connection == wiring.connection)
                .cloned()
                .unwrap_or(wiring)
        })
        .collect();
    Report {
        wirings,
        assessment: first.assessment,
        rehearsed: first.rehearsed,
    }
}

#[cfg(test)]
mod tests;
