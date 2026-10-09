//! Every answer an installed plugin's adapter gave outside its contract, kept beside the
//! install record until a proof the plugin passes clears it.
//!
//! A plugin with an answer kept against a capability fills none of it meanwhile, and a
//! record that cannot be read is no proof that nothing was kept, so it fills nothing
//! either.

use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_contract::client::Witness;

use crate::error::Problem;
use crate::plugin::Nonconforming;

use super::super::Ctx;

/// Every answer kept, or nothing where nowhere is configured to keep them.
///
/// # Errors
///
/// Where the record cannot be read. It is never read as nothing kept.
pub(crate) fn held(ctx: &Ctx) -> Result<Vec<Nonconforming>, Box<Problem>> {
    let Some(at) = kept_at(ctx) else {
        return Ok(Vec::new());
    };
    match std::fs::read_to_string(&at) {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(why) => Err(Box::new(super::unrecorded(&at, &why.to_string()))),
        Ok(text) => serde_json::from_str(&text)
            .map_err(|why| Box::new(super::unrecorded(&at, &why.to_string()))),
    }
}

/// Whether `plugin` may fill `capability`: nothing is kept against it for that
/// capability, and the record says so.
pub(crate) fn fills(ctx: &Ctx, plugin: &str, capability: &str) -> bool {
    held(ctx).is_ok_and(|held| {
        !held
            .iter()
            .any(|one| one.plugin == plugin && one.capability == capability)
    })
}

/// Take every answer kept against `plugin` off the record.
///
/// # Errors
///
/// Where the record cannot be read or written.
pub(crate) fn cleared(ctx: &Ctx, plugin: &str) -> Result<(), Box<Problem>> {
    let mut held = held(ctx)?;
    if !held.iter().any(|one| one.plugin == plugin) {
        return Ok(());
    }
    held.retain(|one| one.plugin != plugin);
    super::super::record::keep(kept_at(ctx).as_deref(), &held)
}

/// Who is told when `plugin`'s adapter answers `capability` outside its contract.
pub(crate) fn witness(ctx: &Ctx, plugin: &str, capability: &str) -> Arc<dyn Witness> {
    Arc::new(Keeping {
        ctx: ctx.clone(),
        plugin: plugin.to_owned(),
        capability: capability.to_owned(),
    })
}

/// Keeps each answer outside the contract against the plugin and capability it was
/// asked as.
struct Keeping {
    ctx: Ctx,
    plugin: String,
    capability: String,
}

impl Witness for Keeping {
    fn nonconforming(&self, operation: &str, why: &str) {
        let answered = Nonconforming {
            plugin: self.plugin.clone(),
            capability: self.capability.clone(),
            operation: operation.to_owned(),
            why: why.to_owned(),
            at: self.ctx.hours_ago(0),
        };
        let _ = kept(&self.ctx, answered);
    }
}

/// Keep `answered` beside every answer kept before it, the latest standing for an
/// operation asked more than once.
///
/// A record that cannot be read is left as it is: written over, it would lose every
/// answer it held.
fn kept(ctx: &Ctx, answered: Nonconforming) -> Result<(), Box<Problem>> {
    let mut held = held(ctx)?;
    held.retain(|one| {
        !(one.plugin == answered.plugin
            && one.capability == answered.capability
            && one.operation == answered.operation)
    });
    held.push(answered);
    super::super::record::keep(kept_at(ctx).as_deref(), &held)
}

/// Where the record is kept: beside the install record.
fn kept_at(ctx: &Ctx) -> Option<PathBuf> {
    super::super::targets::beside_env(ctx, crate::config::paths::NONCONFORMING)
}

#[cfg(test)]
mod tests;
