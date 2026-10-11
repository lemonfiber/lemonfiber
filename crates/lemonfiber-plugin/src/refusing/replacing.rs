//! Which of a plugin's services may stand in the place of one the stack ships.

use crate::schema::Service;
use crate::Violation;

use super::bundled;

/// Whether a service naming the stack service it replaces names its own id, and a service
/// the shipped stack declares.
pub(super) fn replaced(service: &Service, found: &mut Vec<Violation>) {
    let Some(replaces) = service.replaces.as_deref() else {
        return;
    };
    let location = format!("service {}.replaces", service.id);
    if replaces != service.id {
        found.push(Violation {
            location: location.clone(),
            message: format!(
                "names {replaces}, and a service replaces only the stack's own service whose \
                 id it takes, which is its own"
            ),
        });
    }
    if bundled::named(replaces).is_none() {
        found.push(Violation {
            location,
            message: format!("names {replaces}, which the stack lemonfiber ships does not declare"),
        });
    }
}

/// The stack's own service `service` moves in place of, where it names its own id and the
/// shipped stack declares it.
pub(super) fn standing_in(service: &Service) -> Option<&'static lemonfiber_manifest::Service> {
    service
        .replaces
        .as_deref()
        .filter(|replaces| *replaces == service.id)
        .and_then(bundled::named)
}

#[cfg(test)]
mod tests;
