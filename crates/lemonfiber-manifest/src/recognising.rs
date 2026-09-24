//! Which of `stack.toml`'s declarations are names, and which type owns each set.
//!
//! The table and nothing else. How a table is walked, and how a refusal is placed in
//! the manifest's own terms, is [`crate::names`] — shared with the plugin manifest,
//! which asks the same question of a different set of fields.
//!
//! Nothing here holds a list of names either. Each field is handed to the type that
//! owns the set, and the type's own refusal is what gets reported — which is what
//! keeps this from becoming a second copy of an enumeration, silently disagreeing
//! with the first about what a service is allowed to say.

use crate::names::{refused, scan, Closed};
use crate::schema::{ApiKind, Bind, Criticality, HealthKind, KeySource, Protocol};
use crate::Violation;

/// What a profile declares by name.
const ON_PROFILE: &[Closed] = &[Closed {
    at: "protocol",
    reads: refused::<Protocol>,
}];

/// What a service declares by name, its API and its health probe included.
const ON_SERVICE: &[Closed] = &[
    Closed {
        at: "bind",
        reads: refused::<Bind>,
    },
    Closed {
        at: "criticality",
        reads: refused::<Criticality>,
    },
    Closed {
        at: "health.kind",
        reads: refused::<HealthKind>,
    },
    Closed {
        at: "api.kind",
        reads: refused::<ApiKind>,
    },
    Closed {
        at: "api.key_source",
        reads: refused::<KeySource>,
    },
];

/// Every kind of entry a manifest declares, and what each of them declares by name.
const DECLARED: &[(&str, &[Closed])] = &[("profile", ON_PROFILE), ("service", ON_SERVICE)];

/// Everything the manifest declares that this build does not recognise.
///
/// The walk answers with a location and a message; what a fault *is* to this crate is
/// this crate's own, so the pair becomes a [`Violation`] here rather than there.
pub(crate) fn unrecognised(text: &str) -> Vec<Violation> {
    scan(text, DECLARED)
        .into_iter()
        .map(|refusal| Violation {
            location: refusal.location,
            message: refusal.message,
        })
        .collect()
}

#[cfg(test)]
mod tests;
