//! Whether a forwarded port is passed on, and what passing it came to.

use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::condition::Fault;
use crate::error::Severity;
use crate::journal::{Change, Kind};

use super::{CHECK, FIELD, OPERATION, RESOURCE};

/// How a client's port is written where it takes peers on no forwarded port.
const NO_PORT: &str = "none";

/// The kind a port that was not passed is raised as.
const NOT_PASSED: &str = "vpn.port-not-passed";

/// Which pass this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asking {
    /// On the cadence: acted on only where the answer differs from the last.
    Changed,
    /// When the stack starts, or after a reconnect: every client compared with the answer.
    Afresh,
}

/// The guard's last answer every client took.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Last {
    /// The port it forwarded, or none.
    pub port: Option<u16>,
}

/// Whether `answer` is passed on: where it differs from the last, and afresh wherever a
/// port is forwarded at all.
#[must_use]
pub fn acts(asking: Asking, answer: Option<u16>, last: Last) -> bool {
    answer != last.port || (asking == Asking::Afresh && answer.is_some())
}

/// What passing the answer to one client came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CameTo {
    /// It already took peers on it.
    Held,
    /// It was set to it, from what it held where it said.
    Moved {
        /// What it took peers on before, where it answered.
        from: Option<Option<u16>>,
    },
    /// It could not be set.
    Refused {
        /// Why, in the words of whatever refused.
        reason: String,
    },
}

/// Pass `answer` to a client holding `holds`, through `set`.
pub async fn passed<F, E>(answer: Option<u16>, holds: Result<Option<u16>, String>, set: F) -> CameTo
where
    F: FnOnce(Option<u16>) -> E,
    E: Future<Output = Result<(), String>>,
{
    if holds.as_ref().is_ok_and(|held| *held == answer) {
        return CameTo::Held;
    }
    match set(answer).await {
        Ok(()) => CameTo::Moved { from: holds.ok() },
        Err(reason) => CameTo::Refused { reason },
    }
}

/// One client inside the guard, and what passing the port to it came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    /// The id its container runs under.
    pub service: String,
    /// What passing the port came to.
    pub came_to: CameTo,
}

impl Client {
    /// Setting it to `answer`, as the journal records it, where it was set.
    #[must_use]
    pub fn change(&self, answer: Option<u16>, stamp: &str) -> Option<Change> {
        let CameTo::Moved { from } = &self.came_to else {
            return None;
        };
        Some(Change {
            at: stamp.to_owned(),
            operation: OPERATION.to_owned(),
            target: self.service.clone(),
            kind: Kind::Configured {
                resource: RESOURCE.to_owned(),
                id: self.service.clone(),
                field: FIELD.to_owned(),
                previous: from.map(spelled),
                current: spelled(answer),
            },
        })
    }
}

/// One pass of the forwarded port.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pass {
    /// What the guard answered.
    pub answer: Option<u16>,
    /// Every client the answer was passed to, empty where the pass did not act.
    pub clients: Vec<Client>,
    /// Why what was set could not be journalled, where it could not.
    pub unrecorded: Option<String>,
}

impl Pass {
    /// Whether every client took the answer.
    #[must_use]
    pub fn passed(&self) -> bool {
        !self
            .clients
            .iter()
            .any(|client| matches!(client.came_to, CameTo::Refused { .. }))
    }

    /// What was set, as the journal records it.
    #[must_use]
    pub fn changes(&self, stamp: &str) -> Vec<Change> {
        self.clients
            .iter()
            .filter_map(|client| client.change(self.answer, stamp))
            .collect()
    }

    /// The lines an operator reads, one for every client set or not set.
    #[must_use]
    pub fn said(&self) -> Vec<String> {
        self.clients
            .iter()
            .filter_map(|client| said(self.answer, client))
            .chain(self.unrecorded.clone())
            .collect()
    }

    /// The fault every client not set amounts to, where one was not.
    #[must_use]
    pub fn fault(&self) -> Option<Fault> {
        let refused: Vec<String> = self
            .clients
            .iter()
            .filter(|client| matches!(client.came_to, CameTo::Refused { .. }))
            .map(|client| client.service.clone())
            .collect();
        (!refused.is_empty()).then(|| {
            Fault::new(
                NOT_PASSED,
                Severity::Warning,
                &format!(
                    "the forwarded port {} was not passed to {}",
                    spelled(self.answer),
                    refused.join(", ")
                ),
                "peers cannot open connections to a client on another port than the one \
                 forwarded, so it seeds only to peers it reaches itself",
                "Run lemonfiber doctor to see why the client would not take it",
            )
            .caused_by(CHECK)
        })
    }
}

/// A port as the journal and the operator read it.
fn spelled(port: Option<u16>) -> String {
    port.map_or_else(|| NO_PORT.to_owned(), |port| port.to_string())
}

/// What is said about one client, or nothing where it already held the port.
fn said(answer: Option<u16>, client: &Client) -> Option<String> {
    let service = &client.service;
    match (&client.came_to, answer) {
        (CameTo::Held, _) => None,
        (CameTo::Moved { .. }, Some(port)) => Some(format!(
            "{service} takes peers on the forwarded port {port}"
        )),
        (CameTo::Moved { .. }, None) => Some(format!(
            "forwarding ended, so {service} was set back to no forwarded port"
        )),
        (CameTo::Refused { reason }, _) => Some(format!(
            "the forwarded port {} was not passed to {service}: {reason}",
            spelled(answer)
        )),
    }
}

#[cfg(test)]
mod tests;
