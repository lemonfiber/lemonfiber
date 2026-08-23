//! What each step says while it happens.
//!
//! The narration is the operator's mental model being built. Six services are doing
//! things to a file and none of them tells them so; watching it once, narrated, is what
//! turns "it appeared" into "I know what this stack does". So every line is a step, a
//! plain-language phrase, and a detail that is specific enough to be evidence — the
//! number of indexers that answered, the client it went to, the path it landed at.
//!
//! Deliberately not presentation: the phrase and the detail are the product's own words,
//! and where they sit on a screen is the binary's business.

use serde::{Deserialize, Serialize};

use super::Step;

/// One narrated line: a step, and what was specifically true of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Line {
    /// The step being narrated.
    pub step: Step,
    /// What it is doing, in plain language.
    pub said: String,
    /// What was specifically true — the evidence that makes the line worth reading
    /// rather than a spinner. Empty where there is nothing particular to say.
    pub detail: String,
}

impl Line {
    /// A line for `step` with nothing particular to add.
    #[must_use]
    pub fn at(step: Step) -> Self {
        Self {
            step,
            said: step.said().to_owned(),
            detail: String::new(),
        }
    }

    /// A line for `step` with the evidence that makes it worth reading.
    #[must_use]
    pub fn saying(step: Step, detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            ..Self::at(step)
        }
    }

    /// What the search found, told as the two facts that matter: how many indexers
    /// answered, and how many releases came back.
    ///
    /// Both numbers, always. "47 results" alone hides that only one of five indexers
    /// answered, which is the thing an operator needs to know before concluding their
    /// stack works.
    #[must_use]
    pub fn searched(indexers: usize, releases: usize) -> Self {
        Self::saying(
            Step::Searching,
            format!(
                "{indexers} {}, {releases} {}",
                plural(indexers, "indexer", "indexers"),
                plural(releases, "release", "releases")
            ),
        )
    }

    /// Where a release was sent, and over which protocol — the line that teaches the
    /// operator their stack has a download client at all.
    #[must_use]
    pub fn sent_to(client: &str, protocol: &str) -> Self {
        Self::saying(Step::Grabbing, format!("{client}, via {protocol}"))
    }
}

/// How a download is going: how big it is, how fast it is moving, and how long is left.
///
/// Held as numbers rather than a formatted string so the same figures can be narrated,
/// compared against the patience bound, and reported machine-readably without being
/// parsed back out of prose.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Speed {
    /// The download's total size in bytes, where the client reports one.
    pub total: u64,
    /// How many bytes are still to come.
    pub left: u64,
    /// Bytes per second, as most recently reported.
    pub rate: u64,
}

impl Speed {
    /// How long the rest will take at the current rate, or nothing where it is not
    /// moving — an estimate divided by zero is not an estimate.
    #[must_use]
    pub const fn remaining(self) -> Option<std::time::Duration> {
        if self.rate == 0 {
            return None;
        }
        Some(std::time::Duration::from_secs(self.left / self.rate))
    }

    /// Whether this is large enough that its size is worth stating before the wait.
    #[must_use]
    pub const fn is_large(self) -> bool {
        self.total >= super::LARGE
    }

    /// The download narrated: size, rate and estimate, separated the way the rest of the
    /// product separates facts on one line.
    #[must_use]
    pub fn detail(self) -> String {
        let mut parts = vec![size(self.total), format!("{}/s", size(self.rate))];
        if let Some(left) = self.remaining() {
            parts.push(format!("~{}", spell_out(left)));
        }
        parts.join(" · ")
    }

    /// This download as its narrated line.
    #[must_use]
    pub fn line(self) -> Line {
        Line::saying(Step::Downloading, self.detail())
    }
}

/// Something that says each line as it happens.
///
/// The narration is the point of the walkthrough, and a walk that gathered its lines and
/// printed them at the end would be a report — the operator would learn what happened
/// rather than watch it happen, which is a different and much smaller thing. So the
/// running of the walk is handed somewhere to say each line the moment it is true, and
/// where that goes is the caller's business.
pub trait Narrator: Send + Sync {
    /// Say one line, now.
    fn said(&self, line: &Line);
}

/// A size in the decimal gigabytes the rest of the product quotes — the same units the
/// quality presets and the free-space checks use, so two numbers an operator sees on one
/// screen mean the same thing.
#[must_use]
pub fn size(bytes: u64) -> String {
    /// One decimal gigabyte.
    const GB: u64 = 1_000_000_000;
    /// One decimal megabyte.
    const MB: u64 = 1_000_000;

    if bytes >= GB {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a size shown to one decimal place; the lost precision is far below \
                      what is printed"
        )]
        return format!("{:.1} GB", bytes as f64 / GB as f64);
    }
    format!("{} MB", bytes / MB)
}

/// A duration in the coarsest unit that still says something useful — nobody waiting on a
/// download wants it to the second.
#[must_use]
pub fn spell_out(left: std::time::Duration) -> String {
    let seconds = left.as_secs();
    if seconds >= 3600 {
        return format!("{}h", seconds / 3600);
    }
    if seconds >= 60 {
        return format!("{}m", seconds / 60);
    }
    format!("{seconds}s")
}

/// The singular or the plural, because "1 indexers" reads as a bug in the product.
fn plural(count: usize, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 {
        one
    } else {
        many
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Step;
    use super::{size, spell_out, Line, Speed};

    #[test]
    fn a_line_says_what_its_step_says() {
        let line = Line::at(Step::Importing);
        assert_eq!(line.said, Step::Importing.said());
        assert!(line.detail.is_empty(), "nothing particular to add");
    }

    #[test]
    fn a_search_is_narrated_with_both_numbers() {
        // Releases alone would hide that only one of five indexers answered, which is
        // exactly what an operator needs before concluding their stack works.
        assert_eq!(Line::searched(3, 47).detail, "3 indexers, 47 releases");
        assert_eq!(Line::searched(1, 1).detail, "1 indexer, 1 release");
        assert_eq!(Line::searched(0, 0).detail, "0 indexers, 0 releases");
    }

    #[test]
    fn a_grab_names_the_client_and_the_protocol() {
        let line = Line::sent_to("SABnzbd", "usenet");
        assert_eq!(line.step, Step::Grabbing);
        assert_eq!(line.detail, "SABnzbd, via usenet");
    }

    #[test]
    fn a_download_is_narrated_as_size_rate_and_estimate() {
        let speed = Speed {
            total: 2_100_000_000,
            left: 1_680_000_000,
            rate: 14_000_000,
        };
        assert_eq!(speed.detail(), "2.1 GB · 14 MB/s · ~2m");
    }

    #[test]
    fn a_download_that_is_not_moving_is_not_given_an_estimate() {
        let stalled = Speed {
            total: 2_000_000_000,
            left: 2_000_000_000,
            rate: 0,
        };
        assert_eq!(stalled.remaining(), None);
        assert!(!stalled.detail().contains('~'), "no estimate from no rate");
    }

    #[test]
    fn a_large_download_is_known_to_be_large() {
        assert!(Speed {
            total: 20_000_000_000,
            ..Speed::default()
        }
        .is_large());
        assert!(!Speed {
            total: 300_000_000,
            ..Speed::default()
        }
        .is_large());
        assert_eq!(Speed::default().line().step, Step::Downloading);
    }

    #[test]
    fn sizes_read_in_the_units_the_rest_of_the_product_quotes() {
        assert_eq!(size(2_100_000_000), "2.1 GB");
        assert_eq!(size(999_000_000), "999 MB");
        assert_eq!(size(0), "0 MB");
    }

    #[test]
    fn a_wait_is_told_in_the_coarsest_unit_that_still_says_something() {
        assert_eq!(spell_out(Duration::from_secs(45)), "45s");
        assert_eq!(spell_out(Duration::from_secs(120)), "2m");
        assert_eq!(spell_out(Duration::from_secs(7200)), "2h");
    }
}
