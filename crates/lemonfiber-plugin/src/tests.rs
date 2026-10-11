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
                manifest
                    .services
                    .iter()
                    .any(|service| service.speaks.iter().any(|spoken| spoken == capability)),
                "{id} fills {capability} by default and no service of it speaks that"
            );
        }
    }
}
