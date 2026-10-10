//! The household, opened once and kept for a little while.
//!
//! Opening it is the expensive half of asking it anything: the stack's manifest checked
//! against the contract and the recorded admin password read from disk, on every
//! sign-in and every call a member makes. What is kept is the opened household and
//! nothing it said. Every member's call still asks the media server whether they stand,
//! so a member removed there is refused at their next call exactly as before; what the
//! keeping delays is only a stack seeded, or moved, while this surface was serving.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use lemonfiber_core::ports::service::Household;
use tokio::time::Instant;

use super::HouseholdAtHand;

/// How long an opened household is used before it is opened again.
///
/// Ten seconds: a member clicking about makes a call or two a second, and a stack
/// seeded while the surface is up is somebody waiting at most this long to sign in.
pub const KEPT_FOR: Duration = Duration::from_secs(10);

/// A household opened from `asked`, kept for [`KEPT_FOR`].
pub struct Remembered {
    /// Where it is opened from.
    asked: Arc<dyn HouseholdAtHand>,
    /// The one opened last, and when it stops being used.
    held: Mutex<Option<(Instant, Arc<dyn Household>)>>,
}

impl Remembered {
    /// Keep what `asked` opens.
    #[must_use]
    pub fn over(asked: Arc<dyn HouseholdAtHand>) -> Self {
        Self {
            asked,
            held: Mutex::new(None),
        }
    }

    /// The household kept, where it is still to be used at `asked`.
    fn kept(&self, asked: Instant) -> Option<Arc<dyn Household>> {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        held.as_ref()
            .filter(|(until, _)| asked < *until)
            .map(|(_, household)| Arc::clone(household))
    }
}

/// The household kept, or opened again where it has gone stale or there was none.
///
/// Nothing is kept where nothing could be opened, so a stack with no household yet is
/// asked again at the next sign-in rather than answered *none* for the length of the
/// keeping.
#[async_trait::async_trait]
impl HouseholdAtHand for Remembered {
    async fn now(&self) -> Option<Arc<dyn Household>> {
        let asked = Instant::now();
        if let Some(kept) = self.kept(asked) {
            return Some(kept);
        }
        let opened = self.asked.now().await;
        *self.held.lock().unwrap_or_else(PoisonError::into_inner) = opened
            .as_ref()
            .map(|household| (asked + KEPT_FOR, Arc::clone(household)));
        opened
    }

    fn vouches_for(&self, id: &str) -> bool {
        self.asked.vouches_for(id)
    }

    async fn offers_claim(&self, id: &str, token: &str) -> bool {
        self.asked.offers_claim(id, token).await
    }

    fn claim_spent(&self, id: &str) {
        self.asked.claim_spent(id);
    }
}
