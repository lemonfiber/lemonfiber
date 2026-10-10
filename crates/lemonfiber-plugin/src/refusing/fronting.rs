//! Which of a plugin's services may name the API it answers the stack's other services in.

use crate::schema::Service;
use crate::Violation;

/// Whether a service naming its native API names one word, and is the upstream another
/// service of the same plugin fronts.
pub(super) fn native(service: &Service, services: &[Service], found: &mut Vec<Violation>) {
    let Some(native) = service.native.as_deref() else {
        return;
    };
    let location = format!("service {}.native", service.id);
    if !one_word(native) {
        found.push(Violation {
            location: location.clone(),
            message: format!(
                "names `{native}`, and the API a service answers in is named in one lowercase \
                 word"
            ),
        });
    }
    if !fronted(service, services) {
        found.push(Violation {
            location,
            message: "names the API this service answers in, which only the upstream another \
                      service of this plugin fronts may name"
                .to_owned(),
        });
    }
}

/// Whether `name` is one lowercase word: a letter, then letters and digits.
fn one_word(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|one| one.is_ascii_lowercase() || one.is_ascii_digit())
}

/// Whether another of `services` fronts `service`.
fn fronted(service: &Service, services: &[Service]) -> bool {
    services
        .iter()
        .any(|adapter| adapter.id != service.id && adapter.fronts.as_deref() == Some(&service.id))
}

#[cfg(test)]
mod tests;
