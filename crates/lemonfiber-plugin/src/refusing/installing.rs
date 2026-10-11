//! What only a plugin the bundle assembles may declare, which an install and an update
//! refuse.

use crate::schema::Manifest;
use crate::Violation;

/// Everything an install or an update refuses beyond [`super::refusals`]: what only a
/// plugin the bundle assembles may declare.
///
/// A service naming an in-process adapter, and one taking the egress guard's shape with
/// no adapter of its plugin fronting it, are read in a manifest the bundle pins and
/// refused anywhere else.
#[must_use]
pub fn uninstallable(manifest: &Manifest) -> Vec<Violation> {
    let mut found = Vec::new();
    for service in &manifest.services {
        if service.api.is_some() {
            found.push(Violation {
                location: format!("service {}.api", service.id),
                message: "names an in-process adapter, which only a part the bundle assembles \
                          may; a plugin speaks its capabilities' contracts instead"
                    .to_owned(),
            });
        }
        super::shaping::unfronted(service, &manifest.services, &mut found);
    }
    found
}

#[cfg(test)]
pub(crate) mod tests;
