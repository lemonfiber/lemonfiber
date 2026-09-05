//! A limit, in the two ways an operator can express one.
//!
//! Most people do not know what their connection carries in the units a download
//! client asks for. They know they want the stack to have "about half of it" while
//! the house is awake, and they know what happens when it takes all of it. So a
//! limit is expressible as a proportion of what the line was measured to carry as
//! well as an absolute figure, and the measured figure travels with it wherever it
//! is shown — a share without the number it is a share of is a setting nobody can
//! check.
//!
//! A proportion of a line nobody has measured is the one case that must not
//! quietly become something else. It is neither unlimited nor a number, so it is
//! its own answer here and a refusal above.

use serde::{Deserialize, Serialize};

/// The share of the line the download takes when a limit is asked for and no
/// figure is given.
pub const DOWNLOAD_SHARE: u8 = 80;

/// The share the upload takes on the same terms.
///
/// Lower than the download's, and not by taste. Home connections are asymmetric —
/// the uplink is a fraction of the downlink — and a saturated uplink degrades
/// *everything*, downloads included, because the acknowledgements that keep a
/// download moving cannot get out past the queue of upload data.
pub const UPLOAD_SHARE: u8 = 25;

/// How much of the line something may take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", tag = "as", content = "at")]
pub enum Limit {
    /// Nothing holds it back.
    Unlimited,
    /// A proportion of what the line was measured to carry, in whole per cent.
    Share(u8),
    /// A figure in bytes a second, as it was given.
    Absolute(u64),
}

/// What a limit comes to once it is weighed against a measured line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", tag = "is", content = "bytes_per_second")]
pub enum Resolved {
    /// Nothing holds it back.
    Unlimited,
    /// This many bytes a second.
    At(u64),
    /// A proportion was asked for and nothing has measured the line.
    ///
    /// Deliberately not folded into [`Self::Unlimited`]. "Half of an unknown
    /// number" resolving to "no limit at all" is the shape of a setting an
    /// operator believes is in force while the stack takes the whole line.
    Unmeasured,
}

impl Limit {
    /// Read a limit the way an operator writes one.
    ///
    /// A proportion carries the per cent sign, an absolute figure carries a unit
    /// or none, and the words that mean no limit are taken as such. Anything else
    /// is `None` — a limit that could not be read is refused rather than rounded
    /// to something safe-looking, because the safe-looking direction here is the
    /// one that ruins an evening.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        let text = text.trim();
        if matches!(
            text.to_ascii_lowercase().as_str(),
            "unlimited" | "none" | "off"
        ) {
            return Some(Self::Unlimited);
        }
        if let Some(share) = text.strip_suffix('%') {
            let share: u8 = share.trim().parse().ok()?;
            return (1..=100).contains(&share).then_some(Self::Share(share));
        }
        crate::bytes::read(text)
            .filter(|bytes| *bytes > 0)
            .map(Self::Absolute)
    }

    /// What this comes to against a line measured to carry `capacity`.
    ///
    /// A capacity of nothing is no measurement rather than a measurement of
    /// nothing. The two matter: a share of zero would resolve to a limit of zero
    /// bytes a second, which is not an unlimited client — it is a stopped one.
    #[must_use]
    pub fn against(self, capacity: Option<u64>) -> Resolved {
        match self {
            Self::Unlimited => Resolved::Unlimited,
            Self::Absolute(bytes) => Resolved::At(bytes),
            // Scaled before dividing, so a share of a slow line is not rounded to
            // nothing; a line fast enough to overflow this would have to carry
            // more than a hundred exabytes a second.
            Self::Share(share) => capacity
                .filter(|carried| *carried > 0)
                .map_or(Resolved::Unmeasured, |carried| {
                    Resolved::At(carried.saturating_mul(u64::from(share)) / 100)
                }),
        }
    }

    /// Whether this is expressed as a proportion, which is what obliges the
    /// measured figure to be shown beside it.
    #[must_use]
    pub const fn is_share(self) -> bool {
        matches!(self, Self::Share(_))
    }

    /// The limit as it is written and read back.
    ///
    /// A proportion always arrives with the figure it is a proportion *of*, so a
    /// setting can be checked against the line rather than taken on faith. Where
    /// nothing has measured the line the absence is said, not skipped: a share of
    /// an unmeasured line is the one limit that does nothing.
    #[must_use]
    pub fn says(self, capacity: Option<u64>) -> String {
        match self.against(capacity) {
            Resolved::Unlimited => "no limit".to_owned(),
            Resolved::Unmeasured => format!(
                "{} of a line nothing has measured, so nothing is held back",
                self.written()
            ),
            Resolved::At(bytes) => match capacity.filter(|_| self.is_share()) {
                Some(carried) => format!(
                    "{} of {} measured, which is {}",
                    self.written(),
                    crate::bytes::a_second(carried),
                    crate::bytes::a_second(bytes)
                ),
                None => crate::bytes::a_second(bytes),
            },
        }
    }

    /// The limit as it was expressed, without the line it is measured against.
    #[must_use]
    pub fn written(self) -> String {
        match self {
            Self::Unlimited => "unlimited".to_owned(),
            Self::Share(share) => format!("{share}%"),
            Self::Absolute(bytes) => crate::bytes::a_second(bytes),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Limit, Resolved, DOWNLOAD_SHARE, UPLOAD_SHARE};

    /// Ten megabytes a second down, which is an ordinary household line.
    const LINE: u64 = 10 * 1024 * 1024;

    #[test]
    fn a_limit_can_be_a_proportion_as_well_as_a_figure() {
        assert_eq!(Limit::read("50%"), Some(Limit::Share(50)));
        assert_eq!(
            Limit::read(" 2 MiB "),
            Some(Limit::Absolute(2 * 1024 * 1024))
        );
        assert_eq!(Limit::read("unlimited"), Some(Limit::Unlimited));
        assert_eq!(Limit::read("None"), Some(Limit::Unlimited));
        assert_eq!(Limit::read("off"), Some(Limit::Unlimited));
    }

    #[test]
    fn a_proportion_of_the_line_becomes_the_figure_the_client_is_given() {
        assert_eq!(Limit::Share(50).against(Some(LINE)), Resolved::At(LINE / 2));
        assert_eq!(
            Limit::Absolute(1_000).against(Some(LINE)),
            Resolved::At(1_000)
        );
        assert_eq!(Limit::Unlimited.against(Some(LINE)), Resolved::Unlimited);
    }

    #[test]
    fn a_share_of_a_line_nobody_measured_is_its_own_answer() {
        // Not unlimited. "Half of an unknown number" that resolves to "no limit"
        // is a setting the operator believes is in force while the stack takes
        // the whole line, which is the failure this feature exists to stop.
        assert_eq!(Limit::Share(50).against(None), Resolved::Unmeasured);
        assert_ne!(Limit::Share(50).against(None), Resolved::Unlimited);
        // A line measured at nothing is no measurement rather than a measurement
        // of nothing. Resolving it would give a limit of zero bytes a second,
        // which is not an unlimited client but a stopped one.
        assert_eq!(Limit::Share(50).against(Some(0)), Resolved::Unmeasured);
        assert_eq!(Limit::Absolute(1_000).against(None), Resolved::At(1_000));
        assert_eq!(Limit::Unlimited.against(None), Resolved::Unlimited);
    }

    #[test]
    fn a_proportion_is_never_shown_without_the_line_it_is_a_proportion_of() {
        let said = Limit::Share(50).says(Some(LINE));
        assert!(said.contains("50%"), "{said}");
        assert!(said.contains("10.0 MiB/s"), "the measured line: {said}");
        assert!(said.contains("5.0 MiB/s"), "and what it comes to: {said}");
    }

    #[test]
    fn a_proportion_of_nothing_measured_says_that_it_holds_nothing_back() {
        let said = Limit::Share(50).says(None);
        assert!(said.contains("nothing has measured"), "{said}");
        assert!(said.contains("nothing is held back"), "{said}");
    }

    #[test]
    fn an_absolute_limit_reads_as_the_figure_it_is() {
        assert_eq!(Limit::Absolute(LINE).says(None), "10.0 MiB/s");
        assert_eq!(Limit::Absolute(LINE).written(), "10.0 MiB/s");
        assert_eq!(Limit::Unlimited.says(Some(LINE)), "no limit");
        assert_eq!(Limit::Unlimited.written(), "unlimited");
        assert_eq!(Limit::Share(25).written(), "25%");
    }

    #[test]
    fn a_limit_that_could_not_be_read_is_refused_rather_than_rounded() {
        assert_eq!(Limit::read("half"), None);
        assert_eq!(Limit::read("0%"), None);
        assert_eq!(Limit::read("101%"), None);
        assert_eq!(Limit::read("-5%"), None);
        assert_eq!(Limit::read("0"), None, "a limit of nothing is not a limit");
        assert_eq!(Limit::read("5 furlongs"), None);
    }

    #[test]
    fn the_upload_default_is_more_conservative_than_the_download_one() {
        // The requirement, held as a rule rather than as two numbers somebody
        // remembers to keep in order. A saturated uplink degrades downloads too,
        // because acknowledgements cannot get out past the queue of upload data.
        // Read through what each share comes to against one line, rather than
        // compared as two numbers. `UPLOAD_SHARE < DOWNLOAD_SHARE` is settled at
        // compile time and asserts nothing at run time: it would hold just as well
        // against an `against` that ignored both shares and handed back the whole
        // line twice. What being the more careful default comes to is less room on
        // the same connection, which is a figure and can be compared.
        let careful = LINE * u64::from(UPLOAD_SHARE) / 100;
        let generous = LINE * u64::from(DOWNLOAD_SHARE) / 100;
        assert_eq!(
            Limit::Share(UPLOAD_SHARE).against(Some(LINE)),
            Resolved::At(careful)
        );
        assert_eq!(
            Limit::Share(DOWNLOAD_SHARE).against(Some(LINE)),
            Resolved::At(generous)
        );
        assert!(
            careful < generous,
            "the upload default came to {careful} against the download's {generous}"
        );
    }

    #[test]
    fn only_a_proportion_owes_the_measured_figure() {
        assert!(Limit::Share(50).is_share());
        assert!(!Limit::Absolute(1).is_share());
        assert!(!Limit::Unlimited.is_share());
    }
}
