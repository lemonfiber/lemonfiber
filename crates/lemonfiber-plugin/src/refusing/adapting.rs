//! What a plugin's service has to say beside the adapter it names.

use crate::schema::Service;
use crate::Violation;

/// Whether an adapter the service names can reach it.
///
/// The adapter is lemonfiber's and the set is closed, which the schema holds. What the
/// schema cannot hold is the rest of the declaration it needs: where inside the stack's
/// network the service answers, and which version of the shared \*arr shape it speaks.
pub(super) fn adapted(service: &Service, found: &mut Vec<Violation>) {
    let Some(api) = &service.api else {
        return;
    };
    if service.listens.is_none() {
        found.push(Violation {
            location: format!("service {}.listens", service.id),
            message: "is absent, and a service naming an adapter has to say the port it answers \
                      on inside the stack's network, or nothing could reach it there"
                .to_owned(),
        });
    }
    if api.kind == lemonfiber_manifest::ApiKind::Servarr && api.version.is_none() {
        found.push(Violation {
            location: format!("service {}.api.version", service.id),
            message: "is absent, and the servarr shape spans two versions of its API, so which \
                      one this service speaks has to be said"
                .to_owned(),
        });
    }
}

#[cfg(test)]
mod tests;
