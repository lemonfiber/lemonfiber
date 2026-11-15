//! The statuses a service's `http` health path reads up by: an answer, or a refusal of a
//! caller presenting nothing, which tells it is up as surely as an answer does.

use lemonfiber_ports::service::Statuses;

/// The statuses of an answer.
pub(crate) const ANSWERED: Statuses = Statuses { from: 200, to: 399 };

/// The statuses an application answers a caller it will not serve.
const REFUSED: [Statuses; 2] = [Statuses::only(401), Statuses::only(403)];

/// Every status a health path reads up by.
pub(crate) fn up() -> Vec<Statuses> {
    std::iter::once(ANSWERED).chain(REFUSED).collect()
}

/// Whether a health path answering `status` says its service is up.
pub(crate) fn passes(status: u16) -> bool {
    up().iter().any(|statuses| statuses.holds(status))
}
