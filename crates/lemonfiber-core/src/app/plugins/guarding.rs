//! lemonfiber's own guards, the parts filling `invite.decline` and `request.gate`, moved
//! from the stack's own service onto their plugins by the acts that write the project and
//! leave the stack running, with no yes of their own, for good.

use lemonfiber_bundled::{Pinned, Stage, PINNED};
use lemonfiber_contract::capabilities::{invite::decline, request::gate};
use lemonfiber_manifest::Manifest;

use super::following::diagnosed;
use super::moving_in::MoveOffered;
use super::Asked;
use crate::app::Ctx;
use crate::journal::{Change, Kind};
use crate::plugin::{Installed, Installs};
use crate::update::run::Moved;

/// The capabilities a guard fills.
const GUARDED: [&str; 2] = [decline::CAPABILITY, gate::CAPABILITY];

/// Whether `pinned` is one of lemonfiber's own guards the bundle installs.
pub(crate) fn is_guard(pinned: &Pinned) -> bool {
    pinned.stage == Some(Stage::Installed)
        && pinned
            .fills
            .iter()
            .any(|capability| GUARDED.contains(capability))
}

/// Whether `plugin` is one of lemonfiber's own guards the bundle installs.
pub(crate) fn guards(plugin: &str) -> bool {
    PINNED
        .iter()
        .any(|pinned| pinned.first_party.plugin == plugin && is_guard(pinned))
}

/// Whether the operator has given a capability `pinned` fills to another filler, which
/// leaves that guard where it is.
fn given_away(pinned: &Pinned, chosen: &crate::wiring::Chosen) -> bool {
    pinned.fills.iter().any(|capability| {
        chosen
            .filler(capability)
            .is_some_and(|filler| filler != pinned.first_party.plugin)
    })
}

/// The guard moves on offer against `manifest`: each guard the stack still runs as its
/// own service, unless its capability was given to another filler.
pub(crate) fn on_offer(ctx: &Ctx, manifest: &Manifest) -> Vec<MoveOffered> {
    let chosen = crate::app::targets::chosen_fillers(ctx);
    super::moving_in::all_on_offer(ctx, manifest)
        .into_iter()
        .filter(|offered| {
            PINNED.iter().any(|pinned| {
                pinned.first_party.plugin == offered.plugin
                    && is_guard(pinned)
                    && !given_away(pinned, &chosen)
            })
        })
        .collect()
}

/// Move every guard on offer, or on a rehearsal read each move and take none.
///
/// A move taken is the guard's install, journalled as its own run; one refused stays on
/// offer for the next act, saying why.
pub(crate) async fn taken(ctx: &Ctx, manifest: &Manifest) -> Vec<Moved> {
    if ctx.dry_run {
        return rehearsed(ctx, manifest).await;
    }
    let offered = on_offer(ctx, manifest);
    if offered.is_empty() {
        return Vec::new();
    }
    let read = crate::update::run::moving::reading(ctx, offered).await;
    crate::update::run::moving::taken(ctx, read).await
}

/// Every guard move on offer, each read as its install would go and none taken: what an
/// act's rehearsal shows.
pub(crate) async fn rehearsed(ctx: &Ctx, manifest: &Manifest) -> Vec<Moved> {
    crate::update::run::moving::reading(ctx, on_offer(ctx, manifest))
        .await
        .into_iter()
        .map(|one| Moved {
            plugin: one.offered.plugin,
            service: one.offered.service,
            taken: false,
            why: one.agreement.err(),
        })
        .collect()
}

/// The guard moves a plugin act carries: taken once an install, update or removal
/// answering its offer lands, read on a reading or a rehearsal of one, and none for
/// anything else, for an act on a guard itself or for one that did not land.
pub(super) async fn carried(ctx: &Ctx, action: &Asked, installs: &Installs) -> Vec<Moved> {
    let consent = match action {
        Asked::Install { consent, .. }
        | Asked::Update { consent, .. }
        | Asked::Remove { consent, .. } => consent,
        Asked::Installed | Asked::Prove { .. } | Asked::Run { .. } => return Vec::new(),
    };
    let on_a_guard = installs
        .install
        .as_ref()
        .is_some_and(|install| guards(&install.would.plugin))
        || matches!(action, Asked::Remove { plugin, .. } if guards(plugin));
    let Ok(manifest) = super::writing::stack_manifest(ctx) else {
        return Vec::new();
    };
    match (on_a_guard, consent.agreement.is_some()) {
        (false, false) => rehearsed(ctx, &manifest).await,
        (false, true) if ctx.dry_run || landed(installs) => taken(ctx, &manifest).await,
        _ => Vec::new(),
    }
}

/// Whether a plugin act changed the machine: an install or an update recorded, or a
/// removal carried out.
fn landed(installs: &Installs) -> bool {
    installs.install.as_ref().is_some_and(|one| one.recorded)
        || installs
            .update
            .as_ref()
            .is_some_and(|one| one.install.recorded)
        || installs.removal.as_ref().is_some_and(|one| one.removed)
}

/// The journalled change of `would` taking the place of the stack's own guard of its id,
/// under its install's run, where it is one; nothing otherwise.
fn moved_in_place(manifest: &Manifest, would: &Installed) -> Option<Change> {
    let moves = guards(&would.plugin)
        && manifest
            .services
            .iter()
            .any(|service| service.id == would.plugin)
        && would.services.iter().any(|placed| {
            placed.service == would.plugin && placed.replaces.as_deref() == Some(&would.plugin)
        });
    moves.then(|| Change {
        at: would.installed_at.clone(),
        operation: crate::plugin::owner(&would.plugin),
        target: would.plugin.clone(),
        kind: Kind::MovedInPlace {
            service: would.plugin.clone(),
            release: env!("CARGO_PKG_VERSION").to_owned(),
        },
    })
}

/// Write the move `would` takes against `manifest` to the journal, beside the install run
/// it belongs to; nothing where it takes none.
///
/// # Errors
///
/// Where the journal has nowhere to go or cannot be written.
pub(crate) fn journalled(
    ctx: &Ctx,
    manifest: &Manifest,
    would: &Installed,
) -> Result<(), Box<crate::error::Problem>> {
    let Some(moved) = moved_in_place(manifest, would) else {
        return Ok(());
    };
    let paths = crate::app::targets::layout(ctx)
        .ok_or_else(|| diagnosed(&crate::config::store::Failure::Nowhere))?;
    crate::app::recover::journalled(&paths.journal(), std::slice::from_ref(&moved))
        .map_err(|failure| diagnosed(&failure))
}

#[cfg(test)]
mod tests;
