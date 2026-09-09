//! What a request about the household's share of the line was given.
//!
//! Its own file for the reason the setting and the removal beside it have one: seven
//! fields and a reading of them is longer than a variant, and the enum next door is
//! the list of what can be asked rather than the shape of any one request.

/// What a bandwidth request is asking for.
///
/// Every field is what the operator wrote rather than what it means, and is read
/// in the core: a surface that parsed a limit would be a second place the rule
/// about what `50%` means could change, and the three surfaces would then disagree
/// about a household's evening.
///
/// All of it absent is a reading, which is the request every surface makes first
/// and the one nothing is written by.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BandwidthAsked {
    /// The download limit, as a share or a figure.
    pub down: Option<String>,
    /// The upload limit, declared apart from the download's and defaulting lower.
    pub up: Option<String>,
    /// The household's hours, as `HH:MM-HH:MM` on the wall clock.
    pub active: Option<String>,
    /// What the line carries, as `<down>/<up>`, where the operator knows.
    pub line: Option<String>,
    /// A monthly cap on what the stack itself moves.
    pub cap: Option<String>,
    /// What is to happen when that cap is reached, decided in advance.
    pub exceeded: Option<String>,
    /// Lift the limits for this many minutes, and no longer.
    pub unrestricted_for: Option<u64>,
}

impl BandwidthAsked {
    /// Whether this request changes anything, or only asks.
    #[must_use]
    pub const fn anything(&self) -> bool {
        self.down.is_some()
            || self.up.is_some()
            || self.active.is_some()
            || self.line.is_some()
            || self.cap.is_some()
            || self.exceeded.is_some()
            || self.unrestricted_for.is_some()
    }
}
