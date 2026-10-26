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
        digest: None,
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
        dashboard: lemonfiber_manifest::Dashboard::default(),
        media_types: Vec::new(),
        provides: Vec::new(),
        claim: Vec::new(),
        secret: Vec::new(),
        depends_on: Vec::new(),
        grants: Vec::new(),
        host_managed: false,
        memory_mib: None,
        mem_limit_mib: None,
        publish: Vec::new(),
        takes_data: false,
        config_path: None,
        config_ro: false,
        mount: Vec::new(),
        tmpfs: Vec::new(),
        env: std::collections::BTreeMap::new(),
        user_mode: None,
        posture: lemonfiber_manifest::container::Posture::Standard,
        network: None,
        shape: None,
        asks_for: None,
        reaches: None,
        listens: None,
    }
}

/// The stack's guard in front of the library, published to the household.
pub(crate) fn watching() -> Service {
    let mut guard = service("door", Some(Bind::Lan), None);
    guard.provides = vec![lemonfiber_contract::capabilities::stream::guard::CAPABILITY.to_owned()];
    guard
}

/// A plugin bringing a request service, `seerr`, behind an adapter speaking
/// `request.intake`, published to the household through the stack's proxy.
pub(crate) fn asking() -> crate::plugin::Installed {
    fronted("seerr", Some("request.intake@1"), Some("ask"))
}

/// A plugin bringing `id` behind an adapter speaking `speaks` where it names a contract,
/// published to the household through the stack's proxy at `label` where there is one,
/// and reachable from this machine alone where there is not.
pub(crate) fn fronted(
    id: &str,
    speaks: Option<&str>,
    label: Option<&str>,
) -> crate::plugin::Installed {
    let mut adapter =
        crate::test_support::a_placed(&format!("{id}-adapter"), &[], None, Some(8080));
    adapter.fronts = Some(id.to_owned());
    adapter.speaks = speaks.map(str::to_owned).into_iter().collect();
    installed(vec![brought(id, label), adapter])
}

/// A plugin's service, published to the household through the stack's proxy at `label`
/// where there is one, and reachable from this machine alone where there is not.
pub(crate) fn brought(id: &str, label: Option<&str>) -> crate::plugin::Placed {
    let mut placed = crate::test_support::a_placed(id, &[], None, Some(8000));
    placed.name = format!("{id} the plugin's");
    placed.reached = Some(match label {
        Some(label) => crate::plugin::Reached::Household {
            port: 8000,
            hostname: label.to_owned(),
            group: None,
        },
        None => crate::plugin::Reached::Loopback {
            port: 8000,
            group: None,
        },
    });
    placed
}

/// An installed plugin holding `services`.
pub(crate) fn installed(services: Vec<crate::plugin::Placed>) -> crate::plugin::Installed {
    crate::test_support::an_installed("brought", services)
}
