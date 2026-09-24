//! What `SABnzbd` has pulled in a calendar month.
//!
//! This client is the one of the two that can answer the question properly: it
//! keeps a figure per day, keyed by the day, so a month is a sum over the days that
//! fall in it rather than a running total that forgets itself on restart.
//!
//! The days are taken across every account rather than per account. A monthly cap
//! belongs to the line, and two blocks bought from two providers are two allowances
//! over one connection — summing them is the whole point.

use async_trait::async_trait;

use lemonfiber_manifest::Date;

use crate::ports::service::{Failure, Metering, Moved};

use super::accounts::{daily_across, StatsResponse};
use super::Sabnzbd;

#[async_trait]
impl Metering for Sabnzbd {
    async fn moved(&self, month: &str) -> Result<Moved, Failure> {
        let measured: StatsResponse = self
            .read("server_stats", "the account statistics could not be read")
            .await?;
        Ok(Moved {
            down: pulled_in(&daily_across(&measured), month),
            // Usenet is a download and nothing else, so there is no upload to
            // count rather than an upload nobody counted.
            up: 0,
            since_start: false,
        })
    }
}

/// What the days falling in `month` add up to.
///
/// Saturating, so no sum of a provider's own counters can wrap; a day the client
/// dated in a way nothing could place is already absent by the time this sees it.
fn pulled_in(daily: &[(Date, u64)], month: &str) -> u64 {
    daily
        .iter()
        .filter(|(day, _)| falls_in(*day, month))
        .map(|(_, bytes)| *bytes)
        .fold(0, u64::saturating_add)
}

/// Whether a day falls in a `YYYY-MM` month.
fn falls_in(day: Date, month: &str) -> bool {
    written(day) == month.trim()
}

/// A day as the month it falls in.
fn written(day: Date) -> String {
    format!("{:04}-{:02}", day.year, day.month)
}

#[cfg(test)]
mod tests;
