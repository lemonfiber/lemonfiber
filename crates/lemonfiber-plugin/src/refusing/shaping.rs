//! Which of a plugin's services may take a privileged shape.

use crate::schema::{Criticality, Service, Shape};
use crate::Violation;

/// Whether a service asking for a shape is one the shape is for: the upstream an adapter
/// of the same plugin fronts where that adapter speaks the capability the shape serves,
/// or, for `render`, a service providing that capability itself.
pub(super) fn shaped(service: &Service, services: &[Service], found: &mut Vec<Violation>) {
    let Some(shape) = service.shape else {
        return;
    };
    let provides = service.provides.iter().any(|named| named == shape.serves());
    let providing = match shape {
        Shape::Render => provides,
        Shape::EgressGuard => provides && services.len() == 1,
    };
    if !providing && !fronted(service, services, shape) {
        found.push(Violation {
            location: format!("service {}.shape", service.id),
            message: refusal(shape),
        });
    }
}

/// Whether a service taking the egress guard's shape is fronted by an adapter of its
/// plugin, which an install and an update require.
pub(super) fn unfronted(service: &Service, services: &[Service], found: &mut Vec<Violation>) {
    if service.shape == Some(Shape::EgressGuard) && !fronted(service, services, Shape::EgressGuard)
    {
        found.push(Violation {
            location: format!("service {}.shape", service.id),
            message: format!(
                "asks for {} with no adapter of this plugin speaking {} in front of it, which \
                 only a part the bundle assembles may",
                Shape::EgressGuard.name(),
                Shape::EgressGuard.serves()
            ),
        });
    }
}

/// Whether a service declaring `critical` takes the egress guard's shape, which alone
/// may declare it.
pub(super) fn critical(service: &Service, found: &mut Vec<Violation>) {
    if service.criticality == Criticality::Critical && service.shape != Some(Shape::EgressGuard) {
        let unshaped: Vec<&str> = Criticality::UNSHAPED.iter().map(|one| one.name()).collect();
        found.push(Violation {
            location: format!("service {}.criticality", service.id),
            message: format!(
                "is {}, which only a service with shape = \"{}\" may declare; without it: {}",
                Criticality::Critical.name(),
                Shape::EgressGuard.name(),
                unshaped.join(", ")
            ),
        });
    }
}

/// Who may take `shape`, as a refusal says it.
fn refusal(shape: Shape) -> String {
    match shape {
        Shape::EgressGuard => format!(
            "asks for {}, which only the upstream an adapter of this plugin speaking {} fronts, \
             or a plugin's one service providing it, may take",
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
