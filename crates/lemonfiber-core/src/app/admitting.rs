//! Who comes in through the web API, and what this run holds in memory to say so: the
//! sessions it opened, the wrong answers it was given, when each key was last used, the
//! challenges it issued for device keys and the sign-in codes it waits on.

mod answer;
mod attempts;
mod caller;
mod coded;
mod household;
mod keyring;
mod refused;
pub mod sessions;
mod signing_in;

use std::sync::Arc;

pub use answer::Answer;
pub use attempts::Attempts;
pub(crate) use attempts::Door;
pub use caller::{Arrived, Caller, Calling, Carried};
pub use coded::{Coded, SignInCode};
pub use household::{AtHand, Remembered, Stacked, KEPT_FOR};
pub use refused::{held, not_the_password, not_yours, unconfirmed, REFUSALS};
pub use sessions::{Admitted, Opened, Sessions};
pub(crate) use signing_in::{proves_the_member, proves_the_operator};
pub use signing_in::{Offered, PASSWORD};

use super::{Ctx, Outcome};
use crate::admission::Credential;
use crate::device_keys::Challenges;
use crate::error::Problem;
use crate::sign_in_codes::SignInCodes;

/// What this run holds about who may come in, shared by every clone of the context.
pub struct Admitting {
    /// The sessions this run has opened.
    pub sessions: Sessions,
    /// The wrong answers this run has been given.
    pub attempts: Attempts,
    pub(crate) keys: keyring::Keyring,
    pub(crate) challenges: Challenges,
    pub(crate) codes: SignInCodes,
    pub(crate) household: Arc<dyn AtHand>,
}

impl Default for Admitting {
    fn default() -> Self {
        Self::over(Arc::new(Remembered::over(Arc::new(Stacked))))
    }
}

impl Admitting {
    /// A register that finds the household through `household`.
    #[must_use]
    pub fn over(household: Arc<dyn AtHand>) -> Self {
        Self {
            sessions: Sessions::default(),
            attempts: Attempts::default(),
            keys: keyring::Keyring::default(),
            challenges: Challenges::default(),
            codes: SignInCodes::default(),
            household,
        }
    }
}

/// What a sign-in asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// A password exchanged for a session.
    Password(Offered),
    /// A code asked for, or a sign-in by one finished.
    Code(Coded),
    /// Who the session or key a request carried proves it to be.
    Caller(Carried),
}

/// The operator's credential as it is kept now, read on a thread made for blocking.
pub(crate) async fn credential_now(ctx: &Ctx) -> Option<Credential> {
    let kept = ctx.settings.admission.clone()?;
    tokio::task::spawn_blocking(move || crate::admission::at(&kept))
        .await
        .ok()
        .flatten()
}

/// What a sign-in, or a caller's check, comes to.
pub(crate) async fn admitted(ctx: &Ctx, asked: Asked) -> Result<Outcome, Box<Problem>> {
    match asked {
        Asked::Password(offered) => signing_in::signed_in(ctx, &offered)
            .await
            .map(|admitted| Outcome::Admitting(Answer::Admission(admitted))),
        Asked::Code(asked) => coded::coded(ctx, asked).await,
        Asked::Caller(carried) => Ok(Outcome::Admitting(Answer::Caller(
            caller::calling(ctx, &carried, ctx.seams.clock.now()).await,
        ))),
    }
}

#[cfg(test)]
mod tests;
