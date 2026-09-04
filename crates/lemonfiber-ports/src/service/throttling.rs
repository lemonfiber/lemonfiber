//! Telling a download client how much of the line it may take.
//!
//! A write to a download client, and the second thing this product asks one to do
//! rather than to say: letting a completed download go was the first. A figure
//! lemonfiber holds and never hands over is a setting nobody's connection can feel,
//! so a limit has to arrive at the client that would otherwise take the line.
//!
//! Apart from [`super::Client`], which is the wiring shape a service is told about
//! its neighbours in, for the reason [`super::Seeding`] and [`super::Transfers`]
//! are: this is a capability a download client has of its own, and a client that
//! has no upload to limit should not have to pretend to answer for one.
//!
//! Three methods, and the second of them answers with what the first would: a
//! limit is set and read back rather than assumed, because a client that accepts a
//! write and does not apply it looks exactly like one that did.
//!
//! **The window is on the client's own clock.** It carries hours and minutes and
//! no zone, because the client is the thing with a clock set to the household's
//! own — and a schedule expressed against an instant is one that moves an hour
//! twice a year, skipping its boundary in one direction and applying it twice in
//! the other.

use async_trait::async_trait;

use super::Failure;

/// What a client is limited to, or is moving, in bytes a second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rates {
    /// Down, or `None` where nothing holds it back.
    pub down: Option<u64>,
    /// Up, or `None` where nothing holds it back.
    pub up: Option<u64>,
}

/// A stretch of the day on the client's own clock.
///
/// Hours and minutes and nothing else. No date, no zone, no offset — see this
/// module's own note on why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// The hour the household's day starts.
    pub from_hour: u8,
    /// The minute of it.
    pub from_minute: u8,
    /// The hour it ends, which may be the next morning.
    pub to_hour: u8,
    /// The minute of that.
    pub to_minute: u8,
}

/// Which side of the household's day a client's own scheduler has it on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hours {
    /// Inside the household's active hours, so the constrained limits apply.
    Active,
    /// Outside them, where the line is the stack's.
    Quiet,
}

/// What a download client should be held to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wanted {
    /// The rates that apply while the household is awake.
    pub active: Rates,
    /// The rates that apply outside those hours.
    pub quiet: Rates,
    /// The household's hours, or `None` to hold the client to the active rates
    /// around the clock — the conservative direction, and the only honest one for
    /// a client with no scheduler of its own.
    pub window: Option<Window>,
}

/// How a download client answered about the limits on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Throttled {
    /// The limits in force this moment, as the client reports them.
    pub rates: Rates,
    /// Whether this client uploads at all.
    ///
    /// A Usenet client does not, so an upload limit on one is not a limit it
    /// ignored — it is a limit with nothing to apply to, and the two must not be
    /// reported alike.
    pub uploads: bool,
    /// Which side of the household's day its own scheduler has it on, or `None`
    /// where it keeps no schedule to be on a side of.
    pub hours: Option<Hours>,
}

/// Setting and reading a download client's own rate limits.
#[async_trait]
pub trait Throttling: Send + Sync {
    /// What the client is limited to right now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the client is unreachable or refuses.
    async fn throttled(&self) -> Result<Throttled, Failure>;

    /// Hold the client to these rates, and answer with what it reports afterwards.
    ///
    /// The answer is the read-back, not an echo of what was asked: a limit the
    /// client accepted and did not apply is the failure this whole path exists to
    /// notice, and it is invisible to anything that trusts its own request.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the client is unreachable, refuses the change, or
    /// cannot be read afterwards.
    async fn restrain(&self, wanted: &Wanted) -> Result<Throttled, Failure>;

    /// What the client is actually moving this moment, both directions.
    ///
    /// The other half of proving a limit took. A client can report the limit it
    /// was given and move faster than it, and an operator who cannot see that is
    /// an operator who turns the limit off.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the client is unreachable or refuses.
    async fn moving(&self) -> Result<Rates, Failure>;
}

#[cfg(test)]
mod tests {
    use super::{Hours, Rates, Throttled, Wanted, Window};

    #[test]
    fn nothing_held_back_is_absence_rather_than_a_zero() {
        // A client asked for a limit of zero is a client asked to move nothing,
        // which is what a torrent client's own API means by it. No limit at all
        // has to be a different value or the two are one setting.
        assert_eq!(
            Rates::default(),
            Rates {
                down: None,
                up: None
            }
        );
        assert_ne!(
            Rates::default(),
            Rates {
                down: Some(0),
                up: Some(0)
            }
        );
    }

    #[test]
    fn a_client_with_no_scheduler_is_told_apart_from_one_inside_its_hours() {
        let keeping = Throttled {
            rates: Rates::default(),
            uploads: true,
            hours: Some(Hours::Active),
        };
        let keeping_none = Throttled {
            hours: None,
            ..keeping
        };
        assert_ne!(keeping, keeping_none);
        assert_ne!(
            keeping_none,
            Throttled {
                hours: Some(Hours::Quiet),
                ..keeping
            }
        );
    }

    #[test]
    fn a_restraint_with_no_window_is_the_active_rates_around_the_clock() {
        // The conservative direction, and the only honest one for a client with
        // no scheduler: the household is protected all day rather than at no
        // point in it.
        let held = Wanted {
            active: Rates {
                down: Some(1_000),
                up: Some(100),
            },
            quiet: Rates::default(),
            window: None,
        };
        assert!(held.window.is_none());
        assert_eq!(held.active.down, Some(1_000));
        assert_ne!(
            held,
            Wanted {
                window: Some(Window {
                    from_hour: 7,
                    from_minute: 0,
                    to_hour: 23,
                    to_minute: 0,
                }),
                ..held
            }
        );
    }
}
