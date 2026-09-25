use super::{Tier, EVERY};

#[test]
fn every_tier_is_named_by_the_word_that_names_it() {
    assert_eq!(EVERY.len(), 4);
    for tier in EVERY {
        assert_eq!(Tier::named(tier.name()), Some(tier), "{tier:?}");
    }
    assert_eq!(Tier::named("everything"), None);
}

/// The property the whole feature rests on: exactly one tier takes the library,
/// so no other tier can be made to bundle it by a change somewhere else.
#[test]
fn exactly_one_tier_takes_the_operators_own_content() {
    let taking: Vec<Tier> = EVERY
        .into_iter()
        .filter(|tier| tier.takes_media())
        .collect();

    assert_eq!(taking, vec![Tier::Media]);
}

/// And the confirmation is on the same one, read from the same predicate rather
/// than from a second list that could fall out of step with it.
#[test]
fn the_tier_that_takes_media_is_the_tier_that_needs_its_own_agreement() {
    let asking: Vec<Tier> = EVERY
        .into_iter()
        .filter(|tier| tier.needs_its_own_agreement())
        .collect();

    assert_eq!(asking, vec![Tier::Media]);
}

/// Every tier below the media one promises the library survives, in words the
/// operator reads before choosing.
#[test]
fn every_tier_below_media_says_the_library_survives() {
    let silent: Vec<&str> = EVERY
        .into_iter()
        .filter(|tier| !tier.takes_media())
        .filter(|tier| !tier.keeps().to_lowercase().contains("library"))
        .map(Tier::name)
        .collect();

    assert!(
        silent.is_empty(),
        "these are said to keep the media and never say so: {silent:?}"
    );
}

/// Two tiers reach the engine and two do not, which is what keeps an unreachable
/// daemon from being in the way of removing files.
#[test]
fn only_the_tiers_that_need_the_engine_reach_it() {
    assert!(Tier::Stop.touches_containers() && Tier::Services.touches_containers());
    assert!(!Tier::Configuration.touches_containers() && !Tier::Media.touches_containers());
    assert!(Tier::Configuration.touches_configuration());
    assert!(!Tier::Services.touches_configuration());
}

/// Only the removal that takes the images asks what was pulled. Stopping leaves
/// every one of them where it is, so a listing of the whole machine's images is
/// not part of what a stop is agreed to.
#[test]
fn only_the_tier_that_removes_images_asks_what_was_pulled() {
    let asking: Vec<Tier> = EVERY
        .into_iter()
        .filter(|tier| tier.touches_images())
        .collect();

    assert_eq!(asking, vec![Tier::Services]);
}

/// The removal that takes the configuration takes the household with it, and
/// says so before anybody agrees to it. An operator who reads "configuration"
/// and loses every account in the house and every watch state has been told
/// something true and useless.
#[test]
fn the_configuration_removal_says_it_takes_the_household_with_it() {
    let said = Tier::Configuration.removes().to_lowercase();

    assert!(said.contains("signs in with"), "{said}");
    assert!(said.contains("watched"), "{said}");
}

#[test]
fn each_tier_says_what_it_takes_and_what_it_leaves() {
    let quiet: Vec<&str> = EVERY
        .into_iter()
        .filter(|tier| {
            tier.removes().split_whitespace().count() < 5
                || tier.keeps().split_whitespace().count() < 4
        })
        .map(Tier::name)
        .collect();

    assert!(
        quiet.is_empty(),
        "these say too little to choose on: {quiet:?}"
    );
}

#[test]
fn a_tier_is_one_value_that_can_be_compared_and_copied() {
    let chosen = Tier::Services;
    assert_eq!(chosen, chosen);
    assert_ne!(chosen, Tier::Media);
    assert!(format!("{chosen:?}").contains("Services"));
}
