//! What time it is, as the context answers it.
//!
//! Four readings of one port, kept together because they are one question asked in
//! the four shapes the things above need it in: a stamp to write down, a number to
//! subtract, an instant a media server would recognise, and a day the manifest's
//! date rules can be checked against. Split out of [`super`] for the reason the
//! length rule exists — the context was accreting concerns — and this is the one
//! that comes away whole: it touches a single field, and nothing else here reads
//! the clock at all.
//!
//! None of them fail. A clock that will not answer, or one set somewhere a calendar
//! cannot name, reads as the epoch: a machine whose clock is absurd is still a
//! machine an operator is trying to run a stack on, and refusing every dated check
//! because of it would be the worse answer.

use super::Ctx;

impl Ctx {
    /// The moment now, as the opaque stamp durable records carry.
    ///
    /// Seconds since the epoch, read through the clock port rather than from the
    /// system directly, so a test can say what time it is and a record written on
    /// one run can be compared with one written on another.
    pub(crate) fn stamp(&self) -> String {
        self.seconds().to_string()
    }

    /// The same moment as a number, for the records that compare two of them.
    ///
    /// Beside the stamp rather than parsed back out of one: what reads this asks
    /// whether enough time has passed since the last run, and two strings cannot be
    /// subtracted. A clock that will not answer reads as the epoch, which is a machine
    /// that has waited long enough for anything.
    pub(crate) fn seconds(&self) -> u64 {
        self.clock
            .now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or_default()
    }

    /// The moment a given number of hours ago, written as the media server writes
    /// its own records: an ISO-8601 instant ending in `Z`.
    ///
    /// The calendar is left to [`Date::from_unix_seconds`], which already knows
    /// about leap years; only the time of day is arithmetic on what is left over.
    /// Written out rather than reached for from a date library, because this is the
    /// one place in the product that needs an instant rather than a day.
    pub(crate) fn hours_ago(&self, hours: i64) -> String {
        let now = self
            .clock
            .now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
            .unwrap_or_default();
        let then = now.saturating_sub(hours.saturating_mul(3600));
        let day = lemonfiber_manifest::Date::from_unix_seconds(then).unwrap_or(EPOCH);
        let past = then.rem_euclid(86_400);
        let (hour, minute, second) = (past / 3600, (past % 3600) / 60, past % 60);
        format!(
            "{:04}-{:02}-{:02}T{hour:02}:{minute:02}:{second:02}Z",
            day.year, day.month, day.day
        )
    }

    /// Today, as the manifest's date rules mean it.
    ///
    /// A clock before the epoch, or one far enough ahead to overflow a calendar,
    /// falls back to the epoch: refusing to do anything because the machine's clock
    /// is absurd would be a worse answer than checking dates against a date that is
    /// merely wrong.
    pub(crate) fn today(&self) -> lemonfiber_manifest::Date {
        let seconds = self
            .clock
            .now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
            .unwrap_or_default();
        lemonfiber_manifest::Date::from_unix_seconds(seconds).unwrap_or(EPOCH)
    }
}

/// The first day the calendar rules can name, used when the clock cannot be
/// believed at all.
const EPOCH: lemonfiber_manifest::Date = lemonfiber_manifest::Date {
    year: 1970,
    month: 1,
    day: 1,
};
