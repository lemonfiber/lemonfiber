//! A length of time as a person says it.
//!
//! Here rather than in the check that first needed it, for the reason
//! [`crate::bytes`] is: the second thing that has to say how long something has
//! been, or has left, must not spell it differently. An operator meeting "36 hours"
//! in one place and "1.5 days" in another learns the sentences were assembled
//! rather than written.

/// A duration in the words a person uses for it.
///
/// Hours stay hours until two days, because "36 hours" tells an operator more
/// about a stall than "2 days" does; past that the precision stops helping. Below
/// a minute it rounds up rather than down, because "0 minutes" is not an answer to
/// how long something has been going on.
#[must_use]
pub fn duration(seconds: u64) -> String {
    let (count, unit) = match seconds {
        0..=5399 => (seconds.max(60) / 60, "minute"),
        5400..=172_799 => ((seconds.saturating_add(1_800)) / 3_600, "hour"),
        // Saturating for the same reason the count below is: rounding a clock that
        // has gone wrong must not be the thing that brings the run down.
        _ => ((seconds.saturating_add(43_200)) / 86_400, "day"),
    };
    // Saturating rather than `as`: a count large enough to truncate is a clock
    // that has gone wrong, and reporting "1 minute" for it would be worse than
    // reporting an implausibly large number honestly.
    let plural = usize::try_from(count).unwrap_or(usize::MAX);
    format!("{count} {unit}{}", crate::plural::s(plural))
}

#[cfg(test)]
mod tests;
