//! What only a plugin the bundle assembles may declare, which an install and an update
//! refuse.

use crate::schema::Manifest;
use crate::Violation;

/// Everything an install or an update refuses beyond [`super::refusals`]: what only a
/// plugin the bundle assembles may declare.
///
/// A service taking the egress guard's shape with no adapter of its plugin fronting it
/// is read in a manifest the bundle pins and refused anywhere else.
#[must_use]
pub fn uninstallable(manifest: &Manifest) -> Vec<Violation> {
    let mut found = Vec::new();
    for service in &manifest.services {
        super::shaping::unfronted(service, &manifest.services, &mut found);
    }
    found
}

#[cfg(test)]
pub(crate) mod tests;
