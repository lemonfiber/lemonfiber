//! When a run-and-exit service runs: a five-field cron expression, or one of the four
//! shortcuts that stand for one.
//!
//! Read here, beside the manifest, so the reader that refuses a manifest naming a time
//! that is none and the core that fires the times read one grammar.

use serde::{Deserialize, Serialize};

/// The setting naming when a run-and-exit service runs, and the times where it holds
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    /// The setting, as `<plugin>:<name>`.
    pub setting: String,
    /// The times where the setting holds nothing: a five-field cron expression or one of
    /// `@hourly`, `@daily`, `@weekly` and `@monthly`.
    pub default: String,
}

/// The shortcuts a schedule may name, each with the expression it stands for.
pub const SHORTCUTS: [(&str, &str); 4] = [
    ("@hourly", "0 * * * *"),
    ("@daily", "0 0 * * *"),
    ("@weekly", "0 0 * * 0"),
    ("@monthly", "0 0 1 * *"),
];

/// One field's bounds, inclusive.
const BOUNDS: [(u8, u8); 5] = [(0, 59), (0, 23), (1, 31), (1, 12), (0, 7)];

/// A schedule, as the minutes, hours, days of the month, months and days of the week
/// it fires in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cron {
    /// Each field's values, one bit per value.
    fields: [u64; 5],
    /// Whether the day of the month was narrowed rather than written `*`.
    day_narrowed: bool,
    /// Whether the day of the week was narrowed rather than written `*`.
    weekday_narrowed: bool,
}

/// A moment on a wall clock, as a schedule is read against it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallClock {
    /// 0 to 59.
    pub minute: u8,
    /// 0 to 23.
    pub hour: u8,
    /// 1 to 31.
    pub day: u8,
    /// 1 to 12.
    pub month: u8,
    /// 0 to 6, Sunday first.
    pub weekday: u8,
}

impl Cron {
    /// The schedule `text` names, or nothing where it names none.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = SHORTCUTS
            .iter()
            .find(|(shortcut, _)| *shortcut == text)
            .map_or(text, |(_, expression)| *expression);
        let words: Vec<&str> = text.split_whitespace().collect();
        let [minute, hour, day, month, weekday] = words.as_slice() else {
            return None;
        };
        let mut fields = [0; 5];
        for ((field, word), (low, high)) in fields
            .iter_mut()
            .zip([minute, hour, day, month, weekday])
            .zip(BOUNDS)
        {
            *field = values(word, low, high)?;
        }
        let sunday = 1 << 7;
        if fields[4] & sunday != 0 {
            fields[4] = (fields[4] & !sunday) | 1;
        }
        Some(Self {
            fields,
            day_narrowed: *day != "*",
            weekday_narrowed: *weekday != "*",
        })
    }

    /// Whether it fires in the minute `at` names.
    ///
    /// Where both the day of the month and the day of the week are narrowed, either
    /// one firing is enough, as cron has always read them.
    #[must_use]
    pub fn fires(&self, at: WallClock) -> bool {
        let [minutes, hours, days, months, weekdays] = self.fields;
        let has = |field: u64, value: u8| field & (1 << value) != 0;
        let day = has(days, at.day);
        let weekday = has(weekdays, at.weekday);
        let dated = if self.day_narrowed && self.weekday_narrowed {
            day || weekday
        } else {
            day && weekday
        };
        has(minutes, at.minute) && has(hours, at.hour) && has(months, at.month) && dated
    }
}

/// The values one comma-separated field names, one bit each, or nothing where a part
/// is not a value, range or step within `low` to `high`.
fn values(word: &str, low: u8, high: u8) -> Option<u64> {
    let mut held = 0;
    for part in word.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => (range, step.parse::<u8>().ok().filter(|step| *step > 0)?),
            None => (part, 1),
        };
        let (first, last) = if range == "*" {
            (low, high)
        } else if let Some((first, last)) = range.split_once('-') {
            (first.parse().ok()?, last.parse().ok()?)
        } else {
            let one = range.parse().ok()?;
            (one, if part.contains('/') { high } else { one })
        };
        if first < low || last > high || first > last {
            return None;
        }
        for value in (first..=last).step_by(usize::from(step)) {
            held |= 1 << value;
        }
    }
    Some(held)
}

#[cfg(test)]
mod tests;
