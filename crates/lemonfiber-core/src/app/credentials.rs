//! The one place an operator asks what secrets this stack holds.
//!
//! Three asks under one word, and all three answer with the inventory as it now
//! stands: reading it, replacing one line of it, or printing one value. An operator
//! who has just rotated something wants to see the inventory that rotation produced,
//! not a receipt they then have to check against one.
//!
//! Naming a credential is a lookup rather than an identifier, on purpose. What the
//! inventory prints is what the operator types back — matched loosely enough that
//! `jellyfin` finds the administrator password, because a product that prints one string
//! and demands another is one that has to be read twice.
//!
//! Nothing here can answer without the stack, and that is deliberate. Most of the
//! secrets in this stack belong to services, so a manifest that could not be read
//! would leave those reading as none at all — which is a claim, not a gap.

pub(crate) mod certifying;
mod declining;
mod gating;
mod minting;
pub(crate) use minting::upstream_anew;
pub(crate) mod pending;
mod reading;
mod revealing;
mod rotating;
mod tokening;

/// The operation a credential lemonfiber minted is journalled under when a rotation or the
/// settling of one records it.
pub(crate) const OPERATION: &str = "credentials";

use super::targets::project_directory;
use super::{Ctx, Problem};
use crate::app::command::Asking;
use crate::credential::{Held, Inventory, Rotation, Settled};
use crate::error::Diagnose;

pub(crate) use minting::{reminted, Amid};
pub(crate) use rotating::{replace_admin_password, Replacing};

/// What was asked about the credentials this stack holds.
///
/// # Errors
///
/// Returns the [`Problem`] for a stack that could not be read.
pub(crate) async fn credentials(ctx: &Ctx, asked: Asking) -> Result<Inventory, Box<Problem>> {
    let manifest = ctx
        .stack
        .manifest()
        .map_err(|err| Box::new(err.problem()))?;
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    // What is installed is part of the answer, and a record that is there and will not
    // read refuses it: a list of what this stack holds that quietly left a plugin's
    // secrets off would be believed.
    let installed = super::plugins::read(ctx)?;
    let installed = installed.installed();
    // Who fills each of the stack's asks: the media server the gate holds a key in, and
    // who reaches the service whose key a rotation replaces, so every copy of the key is
    // handed the new one and no other.
    let fillers = crate::wiring::Fillers::of(
        &manifest,
        installed,
        &super::targets::chosen_fillers(ctx),
        project.as_deref(),
    )
    .numbered(crate::app::targets::ordinals(ctx));
    let held = reading::taken(ctx, &manifest.services, &fillers, installed).await;

    let inventory = match asked {
        Asking::Read => Inventory::of(held),
        Asking::Reveal {
            credential,
            confirmed,
        } => showing(ctx, held, (&credential, confirmed), &fillers).await,
        Asking::Revoke {
            plugin,
            credential,
            agreement,
        } => {
            let (revoked, after) =
                super::plugins::granting::revoke(ctx, &plugin, &credential, agreement.as_deref())
                    .await?;
            let held = reading::taken(ctx, &manifest.services, &fillers, after.installed()).await;
            return Ok(Inventory::of(held)
                .granting(after.installed())
                .revoking(revoked));
        }
        Asking::Rotate {
            credential,
            agreement,
        } => {
            replacing(
                ctx,
                held,
                (&credential, agreement.as_deref()),
                Reaching {
                    services: &manifest.services,
                    fillers: &fillers,
                    installed,
                },
            )
            .await?
        }
    };
    Ok(inventory.granting(installed))
}

/// The inventory with one value printed, or with the reason there is nothing to print.
async fn showing(
    ctx: &Ctx,
    held: Vec<Held>,
    (credential, confirmed): (&str, bool),
    reaching: &crate::wiring::Fillers,
) -> Inventory {
    let revealed = match named(&held, credential) {
        Some(found) => revealing::reveal(ctx, found, confirmed, reaching).await,
        None => revealing::nothing_by_that_name(credential, &named_ones(&held)),
    };
    Inventory::of(held).showing(revealed)
}

/// The inventory as it stands after a rotation, and what became of that rotation.
///
/// # Errors
///
/// Where the offer a replacement that takes one answers has moved.
async fn replacing(
    ctx: &Ctx,
    held: Vec<Held>,
    (credential, agreement): (&str, Option<&str>),
    reaching: Reaching<'_>,
) -> Result<Inventory, Box<Problem>> {
    let Some(found) = named(&held, credential) else {
        let known = named_ones(&held);
        return Ok(Inventory::of(held).after(unknown(credential, &known)));
    };
    let Reaching {
        services,
        fillers,
        installed,
    } = reaching;
    let mut rotated = if found.setting == certifying::SETTING {
        let stack = crate::seed::run::replacing::Stack { services, fillers };
        certifying::rotate(ctx, found, stack, agreement).await?
    } else {
        rotating::rotate(ctx, found, services, fillers).await
    };
    if matches!(rotated.settled, Settled::Replaced { .. }) {
        rotated
            .consumers
            .extend(crate::settings::run::crossed(ctx, &found.setting).await);
    }
    // Read again, because a landed replacement has changed what the answer is and an
    // inventory taken before it would report the state the rotation just left behind.
    let after = reading::taken(ctx, services, fillers, installed).await;
    Ok(Inventory::of(after).after(rotated))
}

/// The stack a replacement reaches into: its services, who fills each of its asks,
/// where it was written to disk and what is installed beside it.
#[derive(Clone, Copy)]
struct Reaching<'a> {
    /// The stack's own services.
    services: &'a [lemonfiber_manifest::Service],
    /// Who fills each ask, so every copy of a replaced key is found.
    fillers: &'a crate::wiring::Fillers,
    /// What is installed beside the stack.
    installed: &'a [crate::plugin::Installed],
}

/// The line an operator's words name, where they name one.
///
/// Matched on the name it is printed under and on the setting it is recorded as, and
/// loosely: a name typed without its capitals, or a service typed without the words
/// around it, is the same request. Ambiguity resolves to the first match in the order
/// the inventory lists, which is the order it was printed in.
fn named<'a>(held: &'a [Held], asked: &str) -> Option<&'a Held> {
    let asked = asked.trim().to_lowercase();
    if asked.is_empty() {
        return None;
    }
    let asked = crate::config::renamed::asked_now(&asked).unwrap_or(asked);
    held.iter()
        .find(|one| one.name.to_lowercase() == asked || one.setting.to_lowercase() == asked)
        .or_else(|| {
            held.iter()
                .find(|one| one.name.to_lowercase().contains(&asked))
        })
}

/// Every name that would have been accepted, in the order they are listed.
fn named_ones(held: &[Held]) -> Vec<String> {
    held.iter().map(|one| one.name.clone()).collect()
}

/// A rotation that never started, because nothing here answers to that name.
fn unknown(credential: &str, known: &[String]) -> Rotation {
    Rotation::stopped(
        credential,
        Settled::Unknown {
            known: known.to_vec(),
        },
    )
}

#[cfg(test)]
mod tests;
