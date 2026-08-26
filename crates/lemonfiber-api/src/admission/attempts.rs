//! Counting wrong answers, so guessing costs time.
//!
//! The limit is on the surface rather than on whoever is knocking. There is one
//! password here, and a caller choosing a new source address per attempt is the
//! ordinary shape of the attack — so a limit kept per address is a limit an
//! attacker steps around and an operator behind one address runs into.
//!
//! What that costs is that somebody guessing can keep the operator waiting. On a
//! household network that is the better half of the trade: the wait is bounded, it
//! is the same wait for everybody, and the alternative is a password an unbounded
//! number of guesses reaches. The wait is said out loud rather than left to be
//! discovered, so whoever is locked out knows it will end.
//!
//! A few wrong answers are free, because typing one wrongly is what people do.
//! After that each one doubles the wait, up to a cap: doubling is what turns a
//! list of ten thousand common passwords into a wait nobody sits through, and the
//! cap is what stops one afternoon of guessing from locking an operator out for a
//! week.

use std::time::{Duration, SystemTime};

use tokio::sync::Mutex;

/// How many wrong answers cost nothing.
///
/// Three, which is a mistyped password, a forgotten capital, and one more.
const FREE: u32 = 3;

/// The longest wait a wrong answer can earn.
///
/// Five minutes. Long enough that guessing is hopeless — at one attempt per five
/// minutes a list of a thousand takes three and a half days — and short enough that
/// an operator who has been locked out by somebody else's guessing gets back in
/// after a cup of tea rather than after a support request.
const LONGEST: Duration = Duration::from_secs(5 * 60);

/// The most a wait is doubled before the cap decides it.
///
/// Twenty doublings is twelve days, which is past the cap by a very long way, and it
/// is here so the shift is a number the type can hold rather than one that happens to
/// be small enough today.
const BEYOND: u32 = 20;

/// What is known about the wrong answers so far.
#[derive(Default)]
struct Wrong {
    /// How many in a row.
    count: u32,
    /// When the last one arrived.
    last: Option<SystemTime>,
}

/// The wrong answers this run has been given.
#[derive(Default)]
pub struct Attempts {
    /// Behind a lock for the reason the sessions are: two answers can arrive at
    /// once, and a count that lost one of them would be a limit that could be
    /// stepped around by knocking twice.
    wrong: Mutex<Wrong>,
}

impl Attempts {
    /// How long is left before another answer is taken, or nothing where one is.
    pub async fn waiting(&self, now: SystemTime) -> Option<Duration> {
        let wrong = self.wrong.lock().await;
        let last = wrong.last?;
        let since = now.duration_since(last).unwrap_or_default();
        owed(wrong.count)
            .checked_sub(since)
            .filter(|left| !left.is_zero())
    }

    /// Record a wrong one.
    pub async fn wrong(&self, now: SystemTime) {
        let mut wrong = self.wrong.lock().await;
        wrong.count = wrong.count.saturating_add(1);
        wrong.last = Some(now);
    }

    /// Forget them, which a right answer does.
    ///
    /// A right answer is the evidence the wrong ones were somebody's own fingers
    /// rather than somebody guessing, so the next mistake starts from nothing again.
    pub async fn right(&self) {
        let mut wrong = self.wrong.lock().await;
        *wrong = Wrong::default();
    }
}

/// The wait a run of wrong answers has earned.
///
/// Nothing while they are free, then doubling, and never past the cap. Written over
/// the count rather than accumulated, so the wait is a function of what happened
/// rather than of what was recorded — and cannot drift from it.
fn owed(count: u32) -> Duration {
    match count.saturating_sub(FREE) {
        0 => Duration::ZERO,
        past => Duration::from_secs(1u64 << past.min(BEYOND)).min(LONGEST),
    }
}
