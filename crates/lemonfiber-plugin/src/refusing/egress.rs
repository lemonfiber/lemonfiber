//! The service that carries another of its plugin out of the machine, holding nothing: what
//! it names, and that it asks for nothing.

use crate::schema::{Manifest, Service};
use crate::Violation;

/// Whether `service`, where it carries a sibling out of the machine, names another service
/// of `manifest` and makes no ask.
pub(super) fn carried(manifest: &Manifest, service: &Service, found: &mut Vec<Violation>) {
    let Some(sibling) = service
        .network
        .as_ref()
        .and_then(|network| network.egress_for())
    else {
        return;
    };
    let location = format!("service {}.network.egress_for", service.id);
    if !manifest
        .services
        .iter()
        .any(|other| other.id != service.id && other.id == sibling)
    {
        found.push(Violation {
            location: location.clone(),
            message: format!("names {sibling}, which is no other service of this plugin"),
        });
    }
    if manifest.asking.iter().any(|ask| {
        manifest
            .asks(ask.service.as_deref())
            .is_some_and(|asker| asker.id == service.id)
    }) {
        found.push(Violation {
            location,
            message: "makes an ask, and a service carrying another out of the machine holds \
                      nothing"
                .to_owned(),
        });
    }
}

#[cfg(test)]
mod tests;
