//! What a service says leaves the machine: where it reaches and what it asks for there.

use crate::schema::Service;
use crate::Violation;

/// Whether a service says where it reaches and what it asks for there, both or neither,
/// and what it asks for in words.
pub(super) fn stated(service: &Service, found: &mut Vec<Violation>) {
    let refused = |field: &str, message: &str| Violation {
        location: format!("service {}.{field}", service.id),
        message: message.to_owned(),
    };
    match (&service.reaches, &service.asks_for) {
        (Some(_), None) => found.push(refused(
            "asks_for",
            "is missing: a service saying where it reaches says what it asks for there",
        )),
        (None, Some(_)) => found.push(refused(
            "reaches",
            "is missing: a service saying what it asks for says where it reaches",
        )),
        (_, Some(asked)) if asked.trim().is_empty() => found.push(refused(
            "asks_for",
            "is blank: say what it asks for, or that nothing leaves the machine",
        )),
        _ => {}
    }
}

#[cfg(test)]
mod tests;
