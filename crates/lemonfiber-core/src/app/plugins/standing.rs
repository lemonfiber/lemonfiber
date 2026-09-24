//! What the stack's wiring comes to with a plugin in it: what an install would leave
//! contested, and what the operator has chosen a plugin's service to stand in for.
//!
//! Apart from the verbs because both are readings of the wiring rather than steps of an
//! install — one is asked before an install or an update acts, and the other on the
//! read of what is installed.

use crate::error::Problem;
use crate::plugin::{Installed, Register};

use super::super::Ctx;

/// Every capability the operator chose one of these plugins' services to fill.
///
/// Read off the recorded choices, so it answers for what the stack is doing now rather
/// than for what any plugin declared.
pub(super) fn substituted(
    installed: &[Installed],
    chosen: &crate::wiring::Chosen,
) -> Vec<crate::plugin::Substituted> {
    chosen
        .choices()
        .filter_map(|(capability, service)| {
            installed
                .iter()
                .find(|one| one.services.iter().any(|placed| placed.service == service))
                .map(|one| crate::plugin::Substituted {
                    plugin: one.plugin.clone(),
                    capability: capability.to_owned(),
                    service: service.to_owned(),
                })
        })
        .collect()
}

/// Every ask of the stack's that installing this would leave contested.
///
/// Read against the stack as it stands and the plugins already installed, so the answer
/// is about this machine: an ask a plugin installed earlier has already contested is
/// not this install's doing, and is not laid at its door.
///
/// # Errors
///
/// Where the stack's own manifest cannot be read. A rehearsal that could not say what
/// the install would do to the wiring would be stating less than the install does.
pub(super) fn contested(
    ctx: &Ctx,
    held: &Register,
    would: &Installed,
) -> Result<Vec<crate::wiring::Contest>, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(crate::error::Diagnose::problem(&err)))?;
    Ok(crate::wiring::contested_by(
        &manifest,
        held.installed(),
        would,
        &super::super::targets::chosen_fillers(ctx),
    ))
}
