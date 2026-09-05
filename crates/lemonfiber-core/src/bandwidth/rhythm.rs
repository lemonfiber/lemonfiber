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
    pub fn into_day(self) -> u16 {
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
mod tests {
    use super::{Period, Rhythm, Wall, DAY};

    /// The window every case here reads against: awake from seven to eleven.
    fn a_day() -> Rhythm {
        Rhythm {
            from: Wall { hour: 7, minute: 0 },
            to: Wall {
                hour: 23,
                minute: 0,
            },
        }
    }

    /// A wall-clock time from minutes into the day.
    fn at(minutes: u16) -> Wall {
        Wall {
            hour: u8::try_from(minutes / 60).unwrap_or(0),
            minute: u8::try_from(minutes % 60).unwrap_or(0),
        }
    }

    /// A window this file's cases read against, or a day this file already knows,
    /// so a case whose window would not parse fails on its own assertion rather
    /// than quietly reading the wrong window.
    fn window(text: &str) -> Rhythm {
        let read = Rhythm::read(text);
        assert!(read.is_some(), "{text} did not read as a window");
        read.unwrap_or_else(a_day)
    }

    #[test]
    fn a_window_is_read_the_way_a_household_would_write_one() {
        let read = Rhythm::read("07:00-23:00");
        assert_eq!(read, Some(a_day()));
        assert_eq!(
            read.map(|window| window.says()),
            Some("07:00 to 23:00".to_owned())
        );
        assert_eq!(a_day().active_minutes(), 16 * 60);
        assert!(!a_day().wraps());
    }

    #[test]
    fn a_time_in_a_frame_this_cannot_place_is_not_read_at_all() {
        // The whole of what makes a daylight-saving transition a non-event. A
        // window stored against an offset moves an hour twice a year, and the
        // boundary is then either skipped or applied twice — so the offset is
        // refused on the way in rather than dropped on the way through.
        assert_eq!(Wall::read("07:00Z"), None);
        assert_eq!(Wall::read("07:00+02:00"), None);
        assert_eq!(Rhythm::read("07:00+02:00-23:00"), None);
        assert_eq!(Wall::read("7:00"), None, "the shape is fixed-width");
        assert_eq!(Wall::read("24:00"), None);
        assert_eq!(Wall::read("07:60"), None);
        assert_eq!(Wall::read("0700"), None);
        assert_eq!(Wall::read("aa:bb"), None);
        assert_eq!(Rhythm::read("07:00"), None);
        assert_eq!(Rhythm::read("07:00-oops"), None);
        assert_eq!(Rhythm::read("oops-07:00"), None);
    }

    #[test]
    fn a_window_that_starts_where_it_ends_is_refused_rather_than_resolved() {
        // It could as easily mean the whole day as none of it, and a household
        // would find out which by living through an evening.
        assert_eq!(Rhythm::read("07:00-07:00"), None);
    }

    #[test]
    fn the_minutes_the_window_claims_are_the_minutes_it_is_long() {
        // Two independent readings of the same window: the arithmetic that says
        // how long the active hours run, and the classification a client would
        // make minute by minute. They agree or the boundary is applied twice at
        // one end and skipped at the other — the window is closed where it starts
        // and open where it ends, so the two halves of the day meet exactly once.
        for window in [a_day(), window("23:00-07:00"), window("00:00-00:01")] {
            let active = (0..DAY)
                .filter(|minute| window.holds(at(*minute)) == Period::Active)
                .count();
            assert_eq!(active, usize::from(window.active_minutes()), "{window:?}");
            assert!(active > 0 && active < usize::from(DAY), "{window:?}");
        }
    }

    #[test]
    fn a_household_that_is_up_late_gets_a_window_that_runs_past_midnight() {
        let late = window("18:00-02:00");
        assert!(late.wraps());
        assert_eq!(late.active_minutes(), 8 * 60);
        assert_eq!(late.holds(at(19 * 60)), Period::Active);
        assert_eq!(late.holds(at(60)), Period::Active, "after midnight");
        assert_eq!(late.holds(at(3 * 60)), Period::Quiet);
        assert_eq!(late.holds(at(2 * 60)), Period::Quiet, "the end is open");
        assert_eq!(
            late.holds(at(18 * 60)),
            Period::Active,
            "the start is closed"
        );
    }

    #[test]
    fn a_period_says_what_it_means_for_the_house() {
        assert!(Period::Active.means().contains("held to its limits"));
        assert!(Period::Quiet.means().contains("has the line"));
    }

    #[test]
    fn a_window_survives_the_record_it_is_kept_in() {
        let written = serde_json::to_string(&a_day()).unwrap_or_default();
        assert_eq!(written, r#"{"from":"07:00","to":"23:00"}"#);
        assert_eq!(serde_json::from_str::<Rhythm>(&written).ok(), Some(a_day()));
        assert!(serde_json::from_str::<Rhythm>(r#"{"from":"7am","to":"23:00"}"#).is_err());
    }

    #[test]
    fn the_parts_a_clients_own_scheduler_asks_for_come_off_the_wall_clock() {
        assert_eq!(a_day().from.hour(), 7);
        assert_eq!(a_day().to.minute(), 0);
        assert_eq!(a_day().from.to_string(), "07:00");
    }
}
