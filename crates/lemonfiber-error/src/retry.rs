//! Trying again before saying anything.
//!
//! A service that is still starting, a connection reset while a container
//! restarts, a name that resolved a moment late — these fail once and work on the
//! next breath. Reporting them is worse than useless: it teaches an operator that
//! lemonfiber cries wolf, and it buries the failures that are real underneath the
//! ones that were not.
//!
//! So a failure that might be a blip is retried, and only what survives is
//! reported — with how many times it was tried, because "it did not answer" and
//! "it did not answer three times over a second" are different claims and the
//! second is the one worth acting on.
//!
//! Bounded deliberately. A diagnostic that a person is watching must stay
//! responsive, so this is a short flurry rather than exponential backoff to a
//! minute: three attempts inside a second, then the truth. Something that needs
//! longer than that to come up is not a blip, and waiting quietly for it would be
//! its own kind of dishonesty.

use std::time::Duration;

/// How many times to try in total, including the first.
///
/// Three is the smallest number that can tell a blip from a pattern: one is no
/// evidence, two could be coincidence, and by the third the answer is the same
/// answer.
pub const ATTEMPTS: u32 = 3;

/// How long the first retry waits. Each subsequent one waits a multiple of it.
///
/// Linear rather than doubling, and derived from the attempt rather than listed,
/// so the waits and the attempt count cannot drift into a gap where a raised
/// [`ATTEMPTS`] quietly buys no extra tries.
const FIRST_WAIT: Duration = Duration::from_millis(200);

/// Whether something is worth trying again, and how long to wait first.
///
/// `attempt` is how many have been made, so the first failure asks with `1`.
/// `None` once the attempts are spent — the failure is the answer.
#[must_use]
pub fn again(attempt: u32) -> Option<Duration> {
    if attempt == 0 || attempt >= ATTEMPTS {
        return None;
    }
    // Each wait a little longer than the last: a service that needs a moment gets
    // one, without the flurry outgrowing the patience of somebody watching.
    Some(FIRST_WAIT.saturating_mul(attempt))
}

/// Whether a failure reported after this many attempts was persistent, as opposed
/// to something nobody tried twice.
///
/// The distinction an operator acts on: a service that did not answer once may
/// have been busy, and one that did not answer every time it was asked is down.
#[must_use]
pub(crate) const fn is_persistent(attempts: u32) -> bool {
    attempts >= ATTEMPTS
}

/// How a failure that survived reads, so every surface says it the same way.
///
/// Nothing where it was only tried once — claiming persistence for something
/// nobody retried would be the same overstatement in the other direction.
#[must_use]
pub fn said(attempts: u32) -> Option<String> {
    is_persistent(attempts).then(|| {
        format!(
            "still failing after {attempts} attempt{}",
            crate::plural::s(attempts as usize)
        )
    })
}

#[cfg(test)]
mod tests;
