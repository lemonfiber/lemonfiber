//! API shapes a service may declare that this build does not speak.
//!
//! A service that declares no API at all is not here, and the distinction is the whole
//! of what this module is for. The manifest saying nothing is the stack's own statement
//! that there is nothing to integrate with — reporting it back would be telling the
//! operator what they wrote. A service that declares a shape this build cannot answer
//! is the opposite: it is lemonfiber's limit rather than the stack's statement, and
//! until it is said the operator has a service with an API on paper and no feature
//! working against it.
//!
//! Kept out of `app` on purpose. The architecture guard that every declarable API shape
//! is acted on reads the sources under `app` for the name of each shape, so a list of
//! the shapes that are *not* acted on, written there, would satisfy the guard by naming
//! the thing it is looking for. Here it is read as what it is: the exception, written
//! down once, in the one place both the guard and the runtime can ask.

use lemonfiber_manifest::{ApiKind, Service};

use crate::model::UnsupportedReport;

/// The API shapes a service may declare that this build does not speak, and why.
///
/// One, and it is a deferred decision rather than a gap: the book indexer's wiring
/// waits on a live instance to pin its endpoints against. An entry leaves here when the
/// shape is spoken, and the architecture guard reads this list so that the exception is
/// stated once rather than agreed upon twice.
pub const DEFERRED: &[(ApiKind, &str)] = &[(
    ApiKind::Bindery,
    "lemonfiber does not speak this service's API yet — its wiring waits on a live \
     instance to pin the endpoints against — so nothing that has to know what this \
     service is can act on it",
)];

/// Every service in this stack declaring a shape this build does not speak.
///
/// In the order the stack declares them, because that is the order everything else
/// reports services in and a second ordering would be a second list to reconcile.
#[must_use]
pub fn deferred(services: &[Service]) -> Vec<UnsupportedReport> {
    services
        .iter()
        .filter_map(|service| {
            let kind = service.api.as_ref()?.kind;
            DEFERRED
                .iter()
                .find(|(deferred, _)| *deferred == kind)
                .map(|(_, because)| UnsupportedReport {
                    what: service.id.clone(),
                    because: (*because).to_owned(),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests;
