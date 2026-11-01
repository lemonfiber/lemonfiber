//! Whether a start installs the parts this build carries that its forms hold and this
//! machine has never run, as the operator answers it at a terminal.
//!
//! Only an explicit yes installs. A no, a stray return, a run with nobody at a terminal
//! and a machine-readable run each start nothing, and are answered with the report that
//! says what is on offer and how to take it.

use lemonfiber_core::app::{offline, Ctx};
use lemonfiber_core::error::Problem;

/// What is put to the operator when a start would install a part from this build.
pub(crate) const ASK_TO_INSTALL: &str = "Install from this build, offline, and start? [y/N]";

/// What the operator made of the installs a start offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Agreed {
    /// Nothing is on offer, so the start goes ahead as it always has.
    Nothing,
    /// The operator agreed, and this is the offer the start answers.
    Yes(String),
    /// Something is on offer and nobody agreed to it.
    No,
}

/// Put the installs a start of `forms` offers to the operator, where `interactive` says
/// there is one to `ask`.
///
/// # Errors
///
/// Where the forms cannot be resolved.
pub(crate) async fn agreed(
    ctx: &Ctx,
    forms: &[String],
    interactive: bool,
    ask: &dyn Fn(&str) -> String,
) -> Result<Agreed, Box<Problem>> {
    let Some(offline) = offline(ctx, forms).await? else {
        return Ok(Agreed::Nothing);
    };
    if !interactive {
        return Ok(Agreed::No);
    }
    crate::render::moving::installing(&offline.installs, None).print();
    Ok(if is_yes(&ask(ASK_TO_INSTALL)) {
        Agreed::Yes(offline.offer)
    } else {
        Agreed::No
    })
}

/// The offer a start of `forms` answers, as the operator agrees to it at `ask`: none where
/// nothing is on offer or nobody agreed, nor where the forms do not resolve, which the
/// start then refuses in the pre-flight it shares with every lifecycle command.
pub(crate) async fn answered(
    ctx: &Ctx,
    forms: &[String],
    interactive: bool,
    ask: &dyn Fn(&str) -> String,
) -> Option<String> {
    match agreed(ctx, forms, interactive, ask).await {
        Ok(Agreed::Yes(offer)) => Some(offer),
        Ok(Agreed::Nothing | Agreed::No) | Err(_) => None,
    }
}

/// Whether a typed answer agrees.
pub(crate) fn is_yes(said: &str) -> bool {
    matches!(said.trim().to_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests;
