use lemonfiber_manifest::Criticality;

use super::{criticality, profile_of, profiled};
use crate::test_support::{an_installed, media};

/// The stack profile the bundle names for the media server's plugin.
fn pinned_profile() -> Option<&'static str> {
    lemonfiber_bundled::PINNED
        .iter()
        .find(|pin| pin.first_party.plugin == media::PLUGIN)
        .and_then(|pin| pin.profile)
}

#[test]
fn a_plugin_the_bundle_installs_sits_in_the_stack_profile_its_entry_names() {
    let declared: Vec<String> = crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.profiles.into_iter().map(|one| one.id).collect())
        .unwrap_or_default();

    let profile = media::moved().map(|installed| profile_of(&installed));

    assert!(pinned_profile().is_some());
    assert_eq!(profile.as_deref(), pinned_profile());
    assert!(profile.is_some_and(|profile| declared.contains(&profile)));
}

#[test]
fn a_plugin_under_a_first_party_id_that_is_not_the_one_pinned_sits_in_its_own_profile() {
    let stranger = an_installed(media::PLUGIN, Vec::new());

    assert_eq!(profile_of(&stranger), crate::plugin::profile(media::PLUGIN));
    assert!(profiled(&[stranger]).is_empty());
}

#[test]
fn every_service_of_a_plugin_the_bundle_installs_is_a_service_of_its_profile() {
    let found = profiled(&media::moved().into_iter().collect::<Vec<_>>());

    let ids: Vec<&str> = found.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(ids, vec![media::PLUGIN, media::ADAPTER]);
    assert!(found
        .iter()
        .all(|one| Some(one.profile.as_str()) == pinned_profile() && !one.name.is_empty()));
}

#[test]
fn a_service_the_record_no_longer_holds_is_no_service_of_the_profile() {
    let found = profiled(
        &media::moved()
            .map(|mut installed| {
                installed
                    .services
                    .retain(|placed| placed.service == media::ADAPTER);
                installed
            })
            .into_iter()
            .collect::<Vec<_>>(),
    );

    let ids: Vec<&str> = found.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(ids, vec![media::ADAPTER]);
}

#[test]
fn a_plugins_criticality_is_the_stacks_weight_of_the_same_name() {
    for (declared, weighed) in [
        (
            lemonfiber_plugin::Criticality::Critical,
            Criticality::Critical,
        ),
        (lemonfiber_plugin::Criticality::Core, Criticality::Core),
        (
            lemonfiber_plugin::Criticality::Important,
            Criticality::Important,
        ),
        (
            lemonfiber_plugin::Criticality::Enhancing,
            Criticality::Enhancing,
        ),
        (
            lemonfiber_plugin::Criticality::Optional,
            Criticality::Optional,
        ),
    ] {
        assert_eq!(criticality(declared), weighed);
    }
}
