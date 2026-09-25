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
pub(crate) const UPLOAD_SHARE: u8 = 25;

/// How much of the line something may take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case", tag = "as", content = "at")]
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
#[serde(rename_all = "kebab-case", tag = "is", content = "bytes_per_second")]
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
    pub(crate) const fn is_share(self) -> bool {
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
mod tests;
