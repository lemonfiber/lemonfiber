//! A filler telling the core what happened, so the core can tell the household.
//!
//! The core hands a curator or the media server its own address and a key minted for that
//! one filler; from then on the filler sends each happening there as one neutral event,
//! naming the title and never a member, an account or what anybody is watching.

use async_trait::async_trait;

use super::Failure;

/// Asking a filler to tell the core what happened.
#[async_trait]
pub trait Recounting: Send + Sync {
    /// Send each happening to `address` with `key` from now on, replacing whatever it was
    /// sending to before; with neither, send nothing more.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the filler is unreachable or refuses.
    async fn events(&self, address: Option<&str>, key: Option<&str>) -> Result<(), Failure>;
}
