//! Setting the web interface's password, and revoking what the one before vouched for.
//!
//! Asked for twice through the surface's asking port, never carried in a command: nothing
//! here can read a kept password back, so comparing two answers is the only check there
//! is that the first was typed the way it was meant.

use crate::admission::{self as credential, Credential};
use crate::config::store::Failure;
use crate::error::codes::admit::MISTYPED;
use crate::error::{Diagnose as _, Problem, Remedy, State};

use super::{Ctx, Outcome};

/// What is asked above the first answer.
const ASKS: &str = "A password for the web interface:";

/// What is asked above the second.
const AGAIN: &str = "The same password again:";

/// What setting the password came to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct PasswordReport {
    /// Whether every device key registered under the password before was revoked.
    pub device_keys_revoked: bool,
}

/// Ask the operator for a password twice, keep it as the web interface's password, then
/// revoke every device key registered under the one before.
///
/// # Errors
///
/// The [`Problem`] where there is nowhere to keep a password, the two answers differ, the
/// password is too short to be one, the file could not be written, or the device keys
/// could not be swept.
pub(crate) async fn set(ctx: &Ctx) -> Result<Outcome, Box<Problem>> {
    let Some(path) = ctx.settings.admission.as_deref() else {
        return Err(Box::new(Failure::Nowhere.problem()));
    };
    let chosen = ctx.asking.secret(ASKS);
    if chosen != ctx.asking.secret(AGAIN) {
        return Err(Box::new(mistyped()));
    }
    let kept = Credential::set(&chosen, ctx.seams.random.as_ref())
        .map_err(|weak| Box::new(weak.problem()))?;
    credential::keep(path, &kept).map_err(|failure| Box::new(failure.problem()))?;
    crate::device_keys::run::swept(ctx).await?;
    Ok(Outcome::Password(PasswordReport {
        device_keys_revoked: true,
    }))
}

/// The two answers were not the same word.
fn mistyped() -> Problem {
    Problem::new(
        MISTYPED,
        "Those two passwords were not the same",
        "Nothing was changed. The password is asked for twice because nothing here can read \
         one back afterwards, so the second answer is the only check there is that the first \
         one was typed the way it was meant.",
        Remedy::new("Ask for it again"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;
