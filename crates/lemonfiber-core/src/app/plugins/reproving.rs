//! Asking an installed plugin's adapters again whether they speak what they declare,
//! and clearing every answer kept against the plugin where each one does.
//!
//! Named on its own it says what it would ask and what a pass would clear, and asks
//! nothing; answered with the offer that reading named, it asks.

use crate::error::codes::plugin::NOTHING_TO_PROVE;
use crate::error::{Problem, Remedy, State};
use crate::plugin::{Installed, Installs, Register, Reproof};

use super::super::Ctx;
use super::{conformance, offering, proving, Consent};

/// Prove `plugin` again, or say what proving it would ask.
///
/// # Errors
///
/// Where nothing by that name is installed, where what is kept against it cannot be
/// read, where the yes names a reading that has since moved, and where there is no
/// stack its adapters were written into.
pub(crate) async fn prove(
    ctx: &Ctx,
    held: Register,
    plugin: &str,
    consent: &Consent,
) -> Result<Installs, Box<Problem>> {
    let Some(proved) = held
        .installed()
        .iter()
        .find(|one| one.plugin == plugin)
        .cloned()
    else {
        return Err(Box::new(not_installed(plugin, held.installed())));
    };
    let kept: Vec<_> = conformance::held(ctx)?
        .into_iter()
        .filter(|one| one.plugin == plugin)
        .collect();
    let offer = offering::proving(&proved, &kept);
    let acting = offering::acting(ctx, consent, plugin, &offer, &offering::PROVING, &[])?;
    let speaking: Vec<_> = proved
        .services
        .iter()
        .filter(|placed| !placed.speaks.is_empty())
        .collect();
    let proofs: Vec<_> = speaking
        .iter()
        .map(|placed| crate::plugin::speaking(&placed.service))
        .collect();
    let mut proof = Reproof {
        plugin: plugin.to_owned(),
        proofs,
        kept,
        asked: acting,
        cleared: false,
    };
    if !acting {
        return Ok(answering(&held, proof, offer));
    }
    let stack = ctx
        .settings
        .stack_dir
        .as_deref()
        .ok_or_else(|| Box::new(super::writing::nowhere_to_write(plugin)))?;
    let deadline = ctx.seams.clock.now() + ctx.patience;
    for (placed, stated) in speaking.iter().zip(proof.proofs.iter_mut()) {
        stated.came_to = Some(proving::spoken(ctx, stack, &proved, placed, deadline).await);
    }
    proof.cleared = proving::held(&proof.proofs) && conformance::cleared(ctx, plugin).is_ok();
    Ok(answering(&held, proof, offer))
}

/// What proving came to, beside everything installed.
fn answering(held: &Register, proof: Reproof, offer: String) -> Installs {
    Installs {
        nonconforming: Vec::new(),
        proof: Some(proof),
        agreement: Some(offer),
        rehearsed: false,
        installed: held.installed().to_vec(),
        install: None,
        removal: None,
        update: None,
        substituted: Vec::new(),
        sources: Vec::new(),
    }
}

/// Nothing by that name is installed.
fn not_installed(plugin: &str, held: &[Installed]) -> Problem {
    let names: Vec<&str> = held.iter().map(|one| one.plugin.as_str()).collect();
    let meaning = if names.is_empty() {
        "Nothing was asked. This machine has no plugins installed.".to_owned()
    } else {
        format!(
            "Nothing was asked. What is installed: {}.",
            names.join(", ")
        )
    };
    Problem::new(
        NOTHING_TO_PROVE,
        format!("{plugin} is not installed"),
        meaning,
        Remedy::new("Run `lemonfiber plugin installed` to see what is on this machine"),
    )
    .in_state(State::Guided)
}
