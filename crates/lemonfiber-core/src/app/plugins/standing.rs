//! What the stack's wiring comes to with a plugin in it: what an install would leave
//! contested, and what the operator has chosen a plugin's service to stand in for.
//!
//! Apart from the verbs because both are readings of the wiring rather than steps of an
//! install — one is asked before an install or an update acts, and the other on the
//! read of what is installed.

use crate::plugin::{Installed, Register};

use super::super::Ctx;

/// Every capability the operator chose one of these plugins' services to fill.
///
/// Read off the recorded choices, so it answers for what the stack is doing now rather
/// than for what any plugin declared.
pub(crate) fn substituted(
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

/// What installing a plugin would do to the wiring: the asks it would leave contested
/// and the asks its own services would make.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Standing {
    /// Every ask that installing it would leave contested and that is not contested now.
    pub(crate) contests: Vec<crate::wiring::Contest>,
    /// Every ask its services would make, as each would then stand.
    pub(crate) asks: Vec<crate::wiring::Wired>,
}

/// What installing `would` beside `held` would do to the wiring.
///
/// Read against the stack as it stands and the plugins already installed, so the answer
/// is about this machine: an ask a plugin installed earlier has already contested is
/// not this install's doing, and is not laid at its door.
pub(crate) fn standing(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    held: &Register,
    would: &Installed,
) -> Standing {
    let chosen = super::super::targets::chosen_fillers(ctx);
    Standing {
        contests: crate::wiring::contested_by(manifest, held.installed(), would, &chosen),
        asks: crate::wiring::asked_by(manifest, held.installed(), would, &chosen),
    }
}
