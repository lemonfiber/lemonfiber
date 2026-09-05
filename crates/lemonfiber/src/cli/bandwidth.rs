//! What a bandwidth request was asked for.
//!
//! Declared as one thing rather than as seven loose values on a variant, for the
//! reason the bundle's flags are: the same seven would otherwise be written out
//! again by whoever passes them on, and a flag added to one list and not the other
//! is a flag that silently does nothing.
//!
//! Every one of them is carried as the operator wrote it and read in the core. A
//! surface that parsed `50%` here would be a second place the rule about what a
//! share means could change, and three surfaces would then disagree about a
//! household's evening.

use clap::Args;

/// What was asked about the line.
#[derive(Debug, Args)]
pub struct RawBandwidth {
    /// How much of the line downloads may take: a share, a rate, or `unlimited`.
    ///
    /// A share is written with a per-cent sign — `50%` — and is measured against
    /// what the line was last seen to carry, which is shown beside it. A rate is
    /// written as a size a second, as in `2MiB`.
    #[arg(long, value_name = "SHARE OR RATE")]
    pub down: Option<String>,
    /// How much of it uploads may take.
    ///
    /// Set apart from the download and left alone unless you say otherwise. Home
    /// connections are lopsided and a saturated uplink slows everything down,
    /// downloads included, so this defaults to something more careful than the
    /// figure you give above.
    #[arg(long, value_name = "SHARE OR RATE")]
    pub up: Option<String>,
    /// The hours the house is awake, when the limits apply: `07:00-23:00`.
    ///
    /// Outside them the stack has the line. The hours are kept by the download
    /// clients themselves, on your zone's clock, so they follow the wall clock
    /// through the daylight-saving changes. `none` takes the schedule away and
    /// leaves the limits standing around the clock.
    #[arg(long, value_name = "FROM-TO")]
    pub active: Option<String>,
    /// What this line carries, if you know: `60MiB/6MiB`, download first.
    ///
    /// Only needed to set a share before lemonfiber has seen the line at full
    /// speed. It measures as it goes otherwise, from what the downloads achieve
    /// when nothing is holding them back.
    #[arg(long, value_name = "DOWN/UP")]
    pub line: Option<String>,
    /// A monthly allowance for what this stack itself moves: `1TiB`.
    ///
    /// Counts only lemonfiber's own download clients. Everything else in the house
    /// is on the same line and is not counted, so your provider's meter will read
    /// higher. `none` takes the cap away.
    #[arg(long, value_name = "SIZE")]
    pub cap: Option<String>,
    /// What happens when that cap is reached: `pause`, `throttle` or `continue`.
    ///
    /// Chosen now rather than asked at the time, which arrives at two in the
    /// morning on a stack nobody is watching.
    #[arg(long, value_name = "WHAT")]
    pub when_exceeded: Option<String>,
    /// Lift the limits for this many minutes, then put them back.
    ///
    /// For the evening you want something now and know nobody else is affected.
    /// Time-boxed on purpose, so it cannot be left on.
    #[arg(long, value_name = "MINUTES")]
    pub unrestricted_for: Option<u64>,
}
