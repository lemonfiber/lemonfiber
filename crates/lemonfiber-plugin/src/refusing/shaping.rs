//! Which of a plugin's services may take a privileged shape.

use crate::schema::{Service, Shape};
use crate::Violation;

/// Whether a service asking for a shape is one the shape is for: the upstream an adapter
/// of the same plugin fronts where that adapter speaks the capability the shape serves,
/// or, for `render`, a service providing that capability itself.
pub(super) fn shaped(service: &Service, services: &[Service], found: &mut Vec<Violation>) {
    let Some(shape) = service.shape else {
        return;
    };
    let providing =
        shape == Shape::Render && service.provides.iter().any(|named| named == shape.serves());
    if !providing && !fronted(service, services, shape) {
        found.push(Violation {
            location: format!("service {}.shape", service.id),
            message: refusal(shape),
        });
    }
}

/// Who may take `shape`, as a refusal says it.
fn refusal(shape: Shape) -> String {
    match shape {
        Shape::EgressGuard => format!(
            "asks for {}, which only the upstream an adapter of this plugin speaking {} fronts \
             may take",
            shape.name(),
            shape.serves()
        ),
        Shape::Render => format!(
            "asks for {}, which only a service providing {} or the upstream an adapter of this \
             plugin speaking it fronts may take",
            shape.name(),
            shape.serves()
        ),
    }
}

/// Whether another of `services` fronts `service` and speaks a contract of the
/// capability `shape` serves.
fn fronted(service: &Service, services: &[Service], shape: Shape) -> bool {
    services.iter().any(|adapter| {
        adapter.id != service.id
            && adapter.fronts.as_deref() == Some(service.id.as_str())
            && adapter
                .speaks
                .iter()
                .any(|spoken| lemonfiber_contract::capability_of(spoken) == Some(shape.serves()))
    })
}

#[cfg(test)]
mod tests;
