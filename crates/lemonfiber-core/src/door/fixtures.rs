use lemonfiber_manifest::{Api, ApiKind, Bind, Criticality, KeySource, Service};

/// A service as the manifest declares one, varied by the three fields the rule
/// reads.
pub(crate) fn service(id: &str, bind: Option<Bind>, api: Option<ApiKind>) -> Service {
    Service {
        id: id.to_owned(),
        name: id.to_owned(),
        profile: "media".to_owned(),
        image: "image".to_owned(),
        tag: "1".to_owned(),
        port: Some(1),
        bind,
        health: None,
        api: api.map(|kind| Api {
            kind,
            key_source: KeySource::Generated,
            path: None,
            version: None,
        }),
        criticality: Criticality::Important,
        license: "MIT".to_owned(),
        upstream: "https://example.invalid".to_owned(),
        last_release: "2026-01-01".to_owned(),
        describes: "a service".to_owned(),
        without_it: "nothing".to_owned(),
        media_types: Vec::new(),
        provides: Vec::new(),
        depends_on: Vec::new(),
        grants: Vec::new(),
        host_managed: false,
        memory_mib: None,
        asks_for: None,
        reaches: None,
    }
}

/// The request surface, as this stack declares it.
pub(crate) fn asking() -> Service {
    service("seerr", Some(Bind::Lan), Some(ApiKind::Seerr))
}

/// The library, as this stack declares it.
pub(crate) fn watching() -> Service {
    service("jellyfin", Some(Bind::Lan), Some(ApiKind::Jellyfin))
}
