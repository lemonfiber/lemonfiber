//! Further tries at the connections a pass found not answering.

use std::future::Future;
use std::time::Duration;

use crate::error::Problem;
use crate::patience::Patience;
use crate::seed::{Report, State, Wiring, NOT_ANSWERING};

/// How long a service still starting is given to answer a connection it was not answering:
/// a pass again every fifteen seconds for a minute, past a solver's first solve.
pub(super) const UNANSWERED: Patience = Patience {
    between: Duration::from_secs(15),
    asks: 4,
};

/// What `first` came to once every connection it found not answering is tried again by
/// `pass` as `patience` allows, until none is left not answering or the tries run out.
///
/// # Errors
///
/// The problem a later pass stopped on.
pub(super) async fn settled<Passing, Passed>(
    first: Report,
    patience: Patience,
    mut pass: Passing,
) -> Result<Report, Box<Problem>>
where
    Passing: FnMut() -> Passed,
    Passed: Future<Output = Result<Report, Box<Problem>>>,
{
    let mut reported = first;
    for _ in 0..patience.asks {
        if !reported.wirings.iter().any(unanswered) {
            break;
        }
        tokio::time::sleep(patience.between).await;
        let later = pass().await?;
        reported = retried(reported, &later);
    }
    Ok(reported)
}

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
