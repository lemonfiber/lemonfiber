use super::{is_compatible, refusals, Manifest, SCHEMA_VERSION, SUPPORTED_SCHEMA_VERSIONS};

#[test]
fn reads_its_own_schema_version() {
    assert!(is_compatible(SCHEMA_VERSION));
}

#[test]
fn refuses_a_version_it_does_not_support() {
    assert!(!is_compatible(0));
    assert!(!is_compatible(SCHEMA_VERSION + 1));
}

#[test]
fn supports_every_generation_it_has_ever_published() {
    assert!(SUPPORTED_SCHEMA_VERSIONS.contains(&SCHEMA_VERSION));
}

#[test]
fn every_manifest_the_bundle_pins_is_one_this_build_acts_on() {
    for pinned in lemonfiber_bundled::PINNED {
        let id = pinned.first_party.plugin;
        let manifest = Manifest::from_toml(pinned.copy);
        assert!(manifest.is_ok(), "{id}: {manifest:?}");
        let Ok(manifest) = manifest else {
            continue;
        };
        assert_eq!(manifest.plugin.id, id);
        let refused = refusals(&manifest, &[]);
        assert!(refused.is_empty(), "{id}: {refused:?}");
        for capability in pinned.fills {
            assert!(
                manifest.services.iter().any(|service| service
                    .speaks
                    .iter()
                    .chain(&service.provides)
                    .any(|named| named == capability)),
                "{id} fills {capability} by default and no service of it speaks or provides that"
            );
        }
    }
}

#[test]
fn a_speaker_fronting_nothing_is_its_own_upstream_and_speaks_on_its_contract_port() {
    let text = crate::refusing::tests::speaking(r#"["media.serve@1"]"#, "listens     = 8080\n");
    let manifest = Manifest::from_toml(&text);
    assert!(manifest.is_ok(), "{manifest:?}");
    let Ok(mut manifest) = manifest else { return };
    let upstream = |manifest: &Manifest| {
        manifest
            .services
            .first()
            .and_then(|speaker| manifest.upstream_of(speaker))
            .map(|one| one.id.clone())
    };
    assert_eq!(upstream(&manifest).as_deref(), Some("komga-server"));
    assert_eq!(
        manifest
            .services
            .first()
            .and_then(super::Service::contract_port),
        Some(8081)
    );
    for service in &mut manifest.services {
        service.fronts = None;
        service.contract = None;
    }
    assert_eq!(upstream(&manifest).as_deref(), Some("komga"));
    assert_eq!(
        manifest
            .services
            .first()
            .and_then(super::Service::contract_port),
        Some(8080)
    );
    assert_eq!(
        manifest
            .services
            .get(1)
            .and_then(super::Service::contract_port),
        None,
        "it speaks nothing"
    );
    for service in &mut manifest.services {
        service.fronts = Some("nowhere".to_owned());
    }
    assert_eq!(upstream(&manifest), None);
}
