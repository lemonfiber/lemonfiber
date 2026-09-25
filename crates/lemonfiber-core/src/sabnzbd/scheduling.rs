//! Writing the household's hours into `SABnzbd`'s own scheduler.
//!
//! This client keeps a schedule the way a crontab does — a list of dated
//! instruction lines, each one a time, a set of days, an action and its argument —
//! rather than the window with two sets of limits qBittorrent keeps. Everything
//! awkward here follows from that, and every piece of it was learned by driving the
//! pinned image rather than read out of a document.
//!
//! **A line is added and removed, never written over.** The whole list is one
//! setting, and the door that takes the whole list at once splits what it is given
//! on spaces unless it finds a comma — so a single line handed to it becomes six
//! nonsense lines, accepted, saved, and silently ignored by the scheduler
//! afterwards. The client's own configuration pages take one line at a time
//! instead, which is both the safe door and the only one that reloads the running
//! scheduler.
//!
//! **What is not a rate line is left exactly as the operator wrote it.** Their
//! nightly pause, their weekday resume, their server switches and folder scans are
//! none of this errand's business and are never read for more than their action.
//!
//! **What *is* a rate line is replaced.** A household's hours and an operator's own
//! speed schedule are one setting, and two of them in one list is a window
//! overridden at an hour nobody chose — which is worse than either alone. So the
//! rate lines become lemonfiber's, and that is the one thing here that takes
//! something away.

use serde::Deserialize;

use crate::ports::service::{Failure, Hours};

use super::Sabnzbd;

/// The days a household's window runs on, as this client numbers them.
const EVERY_DAY: &str = "1234567";

/// The one action lemonfiber owns a line for.
const RATE: &str = "speedlimit";

/// The first field of a line the client will act on.
const ENABLED: &str = "1";

/// The client's schedule, as `get_config` answers with it.
#[derive(Deserialize)]
struct Held {
    config: Sections,
}

/// The section the schedule lives in.
#[derive(Deserialize)]
struct Sections {
    misc: Schedule,
}

/// The instruction lines themselves.
#[derive(Deserialize)]
struct Schedule {
    #[serde(default)]
    schedlines: Vec<String>,
}

/// One rate-limit line lemonfiber writes: when it fires, and what it sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Turn {
    /// The hour it fires, on the client's own clock.
    pub(super) hour: u8,
    /// The minute of it.
    pub(super) minute: u8,
    /// The limit it sets, in the client's own units.
    pub(super) figure: String,
}

impl Turn {
    /// The line as the client stores it.
    ///
    /// Exactly as the client's own page writes one, down to the field order — the
    /// minute before the hour — because this string is compared against what comes
    /// back, and a line that does not match its stored form is one added again on
    /// every run.
    fn stored(&self) -> String {
        format!(
            "{ENABLED} {} {} {EVERY_DAY} {RATE} {}",
            self.minute, self.hour, self.figure
        )
    }
}

impl Sabnzbd {
    /// The instruction lines the client is holding.
    pub(super) async fn schedule_lines(&self) -> Result<Vec<String>, Failure> {
        let held: Held = self
            .read(
                "get_config&section=misc&keyword=schedlines",
                "the schedule could not be read",
            )
            .await?;
        Ok(held.config.misc.schedlines)
    }

    /// Hold this client to `turns` and to nothing else of lemonfiber's making.
    ///
    /// Idempotent by comparison rather than by rewriting: a run that wants what the
    /// client already holds writes nothing, which matters because every line added
    /// or removed reloads the client's scheduler — and a reload is what makes the
    /// boundary land on the current side of the day rather than at the next one.
    ///
    /// Confirmed by reading the list back, because the pages that take a line
    /// answer with a redirect to themselves and say nothing about what they did.
    pub(super) async fn keeping(&self, turns: &[Turn]) -> Result<Vec<String>, Failure> {
        let wanted: Vec<String> = turns.iter().map(Turn::stored).collect();
        let held = self.schedule_lines().await?;

        for line in held.iter().filter(|line| is_rate(line)) {
            if !wanted.contains(line) {
                self.page("scheduling/delSchedule", &[("line", line.as_str())])
                    .await?;
            }
        }
        for turn in turns.iter().filter(|turn| !held.contains(&turn.stored())) {
            self.page(
                "scheduling/addSchedule",
                &[
                    ("minute", &turn.minute.to_string()),
                    ("hour", &turn.hour.to_string()),
                    ("daysofweek", EVERY_DAY),
                    ("action", RATE),
                    ("arguments", &turn.figure),
                ],
            )
            .await?;
        }

        let after = self.schedule_lines().await?;
        let rates = after.iter().filter(|line| is_rate(line)).count();
        if rates != wanted.len() || !wanted.iter().all(|line| after.contains(line)) {
            return Err(self.endpoint.refused(&format!(
                "the household's hours were written and the client is keeping {rates} rate \
                 instructions instead"
            )));
        }
        Ok(after)
    }
}

/// Whether one stored line is a rate-limit instruction.
///
/// By its action alone. A line's time, days and figure are the operator's business
/// wherever the action is not this one, and reading further would be this errand
/// forming an opinion about a schedule it does not own.
fn is_rate(line: &str) -> bool {
    field(line, 4) == Some(RATE)
}

/// One whitespace-separated field of a stored line, where it has one.
fn field(line: &str, at: usize) -> Option<&str> {
    line.split_whitespace().nth(at)
}

/// Which side of the household's day this client's own schedule has it on.
///
/// Read from what the schedule does and from what is in force this moment, rather
/// than worked out: nothing in this product knows the client's local time of day,
/// and the client is the thing that does. A schedule that never changes the rate
/// puts the client on no side of any day, which is what a client with a single
/// standing limit is honestly on.
///
/// It follows that in the half-minute between a schedule being written and the
/// client's scheduler reloading, this reports the side the client is *still* on.
/// That is the truth about the client, and reporting the side it is about to be on
/// would be reporting a limit that is not yet in force.
pub(super) fn side(lines: &[String], limited: bool) -> Option<Hours> {
    let mut figures: Vec<&str> = lines
        .iter()
        .filter(|line| is_rate(line) && field(line, 0) == Some(ENABLED))
        .filter_map(|line| field(line, 5))
        .collect();
    figures.sort_unstable();
    figures.dedup();
    (figures.len() > 1).then_some(if limited { Hours::Active } else { Hours::Quiet })
}

#[cfg(test)]
mod tests;
