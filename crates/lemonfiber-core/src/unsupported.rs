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
mod tests {
    use super::{deferred, DEFERRED};

    /// One service declaring the given API shape, read back through the manifest
    /// parser rather than built as a struct, so what this calls a declaration is what
    /// the stack's own file means by one.
    ///
    /// A list of one rather than the service itself, and the assertion below is why:
    /// a parse that failed would otherwise need a way out of its own, and a block no
    /// run enters is a line the coverage gate counts against every honest one beside
    /// it. Said here rather than left to the cases, because three of them assert that
    /// nothing is reported — which an empty list satisfies while proving nothing.
    fn service(id: &str, api: &str) -> Vec<lemonfiber_manifest::Service> {
        let written = format!(
            "schema_version = 1\nstack_version = \"0.1.0\"\nmin_cli_version = \"0.1.0\"\n\n\
             [[profile]]\nid = \"tv\"\nname = \"Television\"\ndescription = \"Television\"\n\n\
             [[service]]\nid = \"{id}\"\nname = \"{id}\"\nprofile = \"tv\"\n\
             image = \"example/{id}\"\ntag = \"1.0.0\"\n\
             criticality = \"core\"\nlicense = \"GPL-3.0-only\"\n\
             upstream = \"https://example.invalid/{id}\"\nlast_release = \"2026-01-01\"\n\
             describes = \"Does a thing\"\nwithout_it = \"Do the thing yourself\"\n{api}"
        );
        let read: Vec<_> = lemonfiber_manifest::Manifest::from_toml(&written)
            .ok()
            .map(|manifest| manifest.services)
            .unwrap_or_default();
        assert_eq!(
            read.len(),
            1,
            "a manifest this test wrote is one the parser reads: {written}"
        );
        read
    }

    #[test]
    fn a_service_declaring_a_shape_this_build_does_not_speak_is_named_with_why() {
        let theirs = service(
            "bookish",
            "\n[service.api]\nkind = \"bindery\"\nkey_source = \"generated\"\n",
        );

        let found = deferred(&theirs);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found.first().is_some_and(|one| one.what == "bookish"),
            "{found:?}"
        );
        assert!(
            found.first().is_some_and(|one| one.because.contains("yet")),
            "{found:?}"
        );
    }

    /// The manifest saying nothing is the stack's own statement, not lemonfiber's
    /// limit, so it is not reported back at the operator who wrote it.
    #[test]
    fn a_service_declaring_no_api_at_all_is_not_reported() {
        assert!(deferred(&service("caddy", "")).is_empty());
    }

    #[test]
    fn a_service_declaring_a_shape_this_build_does_speak_is_not_reported() {
        let ours = service(
            "sonarr",
            "\n[service.api]\nkind = \"servarr\"\nkey_source = \"config-xml\"\n\
             path = \"/config/config.xml\"\nversion = 3\n",
        );
        assert!(deferred(&ours).is_empty());
    }

    /// Every entry says why, because the report carries the sentence rather than the
    /// name — an entry with no words is a service named as unsupported and nothing an
    /// operator can do about it.
    #[test]
    fn every_deferred_shape_says_why_it_is_deferred() {
        let silent: Vec<&str> = DEFERRED
            .iter()
            .filter(|(_, because)| because.split_whitespace().count() < 6)
            .map(|(_, because)| *because)
            .collect();
        assert!(silent.is_empty(), "{silent:?}");
    }
}
