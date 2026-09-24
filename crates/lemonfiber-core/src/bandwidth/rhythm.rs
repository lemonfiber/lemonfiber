//! The household's day: the hours people are awake and using the line.
//!
//! Not cron. The useful shape is the one every household already has — constrained
//! while people are up, unconstrained overnight — so what is declared here is a
//! single stretch of the day, once, for every download client rather than twice in
//! two clients' different dialects.
//!
//! **A time here is a time on the wall clock and never an instant.** No zone, no
//! offset, no date. That is what makes a daylight-saving transition a non-event:
//! "quiet hours start at 23:00" means the moment the clock on the wall reads
//! 23:00, whichever of the two 01:30s a household is living through. A schedule
//! stored as an offset from an instant is the one that shifts an hour twice a year
//! and either skips its boundary or applies it twice — which is why an offset is
//! refused on the way in rather than dropped on the way through, the same reading
//! [`crate::instant`] makes of a timestamp in a frame it cannot place.
//!
//! Which side of the boundary the stack is on right now is not answered here.
//! Nothing in this product knows the household's local time of day — it holds
//! instants and calendar days with nothing between them — so the window is handed
//! to a client whose own clock is set to the household's zone, and which period is
//! in force is read back from that client rather than computed here.

use serde::{Deserialize, Serialize};

/// Minutes in a day, which is what a wall clock wraps at.
const DAY: u16 = 24 * 60;

/// A time on the wall clock: no zone, no offset, no date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Wall {
    /// Hour of the day, 0 to 23.
    hour: u8,
    /// Minute of the hour, 0 to 59.
    minute: u8,
}

impl Wall {
    /// Read a `HH:MM` time, and nothing else.
    ///
    /// Strict about shape, and strict for a reason beyond tidiness: a trailing `Z`
    /// or `+02:00` is a time in a frame this does not read, and taking the digits
    /// in front of it would silently place the boundary an hour or two from where
    /// the household put it.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        let (hour, minute) = text.trim().split_once(':')?;
        let shaped = hour.len() == 2
            && minute.len() == 2
            && [hour, minute]
                .iter()
                .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()));
        if !shaped {
            return None;
        }
        let hour: u8 = hour.parse().ok()?;
        let minute: u8 = minute.parse().ok()?;
        (hour < 24 && minute < 60).then_some(Self { hour, minute })
    }

    /// How far into the day this is, in minutes.
    #[must_use]
    pub(crate) fn into_day(self) -> u16 {
        u16::from(self.hour) * 60 + u16::from(self.minute)
    }

    /// The hour, as a client's own scheduler wants it.
    #[must_use]
    pub const fn hour(self) -> u8 {
        self.hour
    }

    /// The minute, as a client's own scheduler wants it.
    #[must_use]
    pub const fn minute(self) -> u8 {
        self.minute
    }
}

impl std::fmt::Display for Wall {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:02}:{:02}", self.hour, self.minute)
    }
}

impl From<Wall> for String {
    fn from(wall: Wall) -> Self {
        wall.to_string()
    }
}

impl TryFrom<String> for Wall {
    type Error = &'static str;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::read(&text).ok_or("a wall-clock time is written HH:MM, with no zone")
    }
}

/// Which side of the household's day a moment falls on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Period {
    /// People are up and using the line, so the stack is held back.
    Active,
    /// The house is asleep and the line is the stack's.
    Quiet,
}

impl Period {
    /// What this period means for the household, in the words it is shown in.
    #[must_use]
    pub const fn means(self) -> &'static str {
        match self {
            Self::Active => "people are using the line, so the stack is held to its limits",
            Self::Quiet => "the house is asleep, so the stack has the line",
        }
    }
}

/// The hours the household is awake, declared once for every download client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Rhythm {
    /// When the household's day starts.
    #[schemars(with = "String")]
    pub from: Wall,
    /// When it ends, which may be the next morning.
    #[schemars(with = "String")]
    pub to: Wall,
}

impl Rhythm {
    /// Read a `HH:MM-HH:MM` stretch of the day.
    ///
    /// A window that starts where it ends is refused rather than resolved. It
    /// could as easily mean the whole day as none of it, and a household would
    /// find out which by living through an evening.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        let (from, to) = text.trim().split_once('-')?;
        let from = Wall::read(from)?;
        let to = Wall::read(to)?;
        (from != to).then_some(Self { from, to })
    }

    /// Which period a wall-clock time falls in.
    ///
    /// Half-open at the end, so the minute the active hours close is already quiet
    /// and no minute of the day belongs to both. A window running past midnight is
    /// the ordinary case for a household that is up late, and is read as such
    /// rather than as a mistake.
    #[must_use]
    pub fn holds(&self, at: Wall) -> Period {
        let (from, to, when) = (self.from.into_day(), self.to.into_day(), at.into_day());
        let inside = if from < to {
            when >= from && when < to
        } else {
            when >= from || when < to
        };
        if inside {
            Period::Active
        } else {
            Period::Quiet
        }
    }

    /// Whether the household's day runs past midnight.
    #[must_use]
    pub fn wraps(&self) -> bool {
        self.from.into_day() > self.to.into_day()
    }

    /// How long the active hours run, in minutes.
    #[must_use]
    pub fn active_minutes(&self) -> u16 {
        let (from, to) = (self.from.into_day(), self.to.into_day());
        if from < to {
            to - from
        } else {
            DAY - from + to
        }
    }

    /// The window as it is written and read back.
    #[must_use]
    pub fn says(&self) -> String {
        format!("{} to {}", self.from, self.to)
    }
}

#[cfg(test)]
mod tests;
