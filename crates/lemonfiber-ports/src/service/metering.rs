//! What a download client has moved, for counting against a declared cap.
//!
//! Apart from [`super::Throttling`] because it is the opposite question: that one
//! asks how fast the client may go, and this asks how far it has gone. A client
//! could answer either without the other, and a household on an unmetered line
//! asks only the first.
//!
//! The two clients count differently and the difference is carried rather than
//! smoothed over. One keeps a figure per calendar day and can answer for a month
//! exactly; the other keeps a running total since it last started, which is an
//! under-count by however much it moved before the last restart. A cap report that
//! presented those alike would be a report an operator could not act on, because
//! the direction of its error would be invisible.

use async_trait::async_trait;

use super::Failure;

/// What one download client has moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Moved {
    /// Bytes pulled down.
    pub down: u64,
    /// Bytes given back. Zero for a client that does not upload.
    pub up: u64,
    /// Whether this is only what has moved since the client last started, rather
    /// than the month that was asked for.
    ///
    /// True is an under-count of unknown size, and it must reach the report as
    /// such: a cap that reads three-quarters spent when the provider's own meter
    /// says nearly all of it is worse than no figure, because it is believed.
    pub since_start: bool,
}

/// Reading what a download client has moved.
#[async_trait]
pub trait Metering: Send + Sync {
    /// What this client moved in the calendar month `month`, written `YYYY-MM`.
    ///
    /// A client that cannot count by month answers with what it can and sets
    /// [`Moved::since_start`], rather than answering for a different period as
    /// though it were the one asked for.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the client is unreachable or refuses.
    async fn moved(&self, month: &str) -> Result<Moved, Failure>;
}

#[cfg(test)]
mod tests {
    use super::Moved;

    #[test]
    fn a_count_that_is_only_since_the_last_restart_is_not_the_same_figure() {
        // The direction of the error has to be visible, or the report is one an
        // operator cannot act on.
        let month = Moved {
            down: 100,
            up: 10,
            since_start: false,
        };
        assert_ne!(
            month,
            Moved {
                since_start: true,
                ..month
            }
        );
        assert_eq!(Moved::default().down, 0);
    }
}
