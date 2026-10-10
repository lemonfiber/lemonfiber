//! Which of a plugin's services may take a privileged shape.

use crate::schema::{Service, Shape};
use crate::Violation;

/// Whether a service asking for a shape is the upstream an adapter of the same plugin
/// fronts, where that adapter speaks the capability the shape is for.
pub(super) fn shaped(service: &Service, services: &[Service], found: &mut Vec<Violation>) {
    let Some(shape) = service.shape else {
        return;
    };
    if !fronted(service, services, shape) {
        found.push(Violation {
            location: format!("service {}.shape", service.id),
            message: format!(
                "asks for {}, which only the upstream an adapter of this plugin speaking {} \
                 fronts may take",
                shape.name(),
                shape.fronted_by()
            ),
        });
    }
}

/// Whether another of `services` fronts `service` and speaks a contract of the
/// capability `shape` is for.
fn fronted(service: &Service, services: &[Service], shape: Shape) -> bool {
    services.iter().any(|adapter| {
        adapter.id != service.id
            && adapter.fronts.as_deref() == Some(service.id.as_str())
            && adapter.speaks.iter().any(|spoken| {
                lemonfiber_contract::capability_of(spoken) == Some(shape.fronted_by())
            })
    })
}

#[cfg(test)]
mod tests;
