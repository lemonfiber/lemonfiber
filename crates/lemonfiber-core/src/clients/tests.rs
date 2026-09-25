use super::{
    guidance, Straining, Support, A_LIGHTER_PRESET, DEVICES, NOTHING_IS_INSTALLED, ONLY_AT_HOME,
    PLAYBACK_WILL_STRUGGLE, TROUBLE,
};
use crate::platform::Environment;
use crate::quality::Preset;
use crate::transcoding::{warn_before_confirming, Playback};
use crate::wizard::Library;

/// Every cause says how to tell it from the others under its symptom.
///
/// The requirements ask that the causes be told apart, not listed. A cause with
/// no way to tell it from its neighbour is a guess offered as an answer, and a
/// reader given three of those is no better off than with none.
#[test]
fn every_cause_says_how_to_tell_it_from_the_others() {
    let silent: Vec<&str> = TROUBLE
        .iter()
        .flat_map(|one| one.causes)
        .filter(|cause| cause.tell.split_whitespace().count() < 4)
        .map(|cause| cause.because)
        .collect();

    assert!(
        silent.is_empty(),
        "these give no way to tell them apart: {silent:?}"
    );
    assert!(
        !TROUBLE.is_empty(),
        "no symptom is answered, so this checked nothing"
    );
}

/// Every cause says what is wrong and what to do about it.
#[test]
fn every_cause_carries_a_fix() {
    let thin: Vec<&str> = TROUBLE
        .iter()
        .flat_map(|one| one.causes)
        .filter(|cause| cause.fix.is_empty() || cause.because.is_empty())
        .map(|cause| cause.because)
        .collect();

    assert!(thin.is_empty(), "these name no fix: {thin:?}");
}

/// A symptom with one cause needs no telling apart, and one with several does.
///
/// Both shapes are here on purpose: a guest network has one cause and an app that
/// cannot find the server has three. What would be wrong is a symptom with none.
#[test]
fn every_symptom_offers_at_least_one_cause() {
    let empty: Vec<&str> = TROUBLE
        .iter()
        .filter(|one| one.causes.is_empty())
        .map(|one| one.symptom)
        .collect();

    assert!(empty.is_empty(), "these answer nothing: {empty:?}");
    assert!(
        TROUBLE.iter().any(|one| one.causes.len() > 1),
        "no symptom has causes to tell apart, so the rule above is checking nothing"
    );
}

/// A browser is present, and it is the entry that needs nothing installed.
#[test]
fn a_browser_is_offered_and_needs_nothing_installed() {
    let always: Vec<&super::Device> = DEVICES
        .iter()
        .filter(|one| one.support == Support::Fallback)
        .collect();

    assert_eq!(
        always.len(),
        1,
        "one entry always works, and it is the browser"
    );
    let asks_for_an_app: Vec<&str> = always
        .iter()
        .filter(|one| !one.client.contains("no app"))
        .map(|one| one.device)
        .collect();
    assert!(asks_for_an_app.is_empty(), "{asks_for_an_app:?}");
    let not_a_browser: Vec<&str> = always
        .iter()
        .filter(|one| !one.device.to_lowercase().contains("browser"))
        .map(|one| one.device)
        .collect();
    assert!(not_a_browser.is_empty(), "{not_a_browser:?}");
}

/// Every device marked [`Support::Poor`] carries an alternative, and at least
/// one device is so marked — without the second half this passes on an empty
/// filter.
#[test]
fn a_device_that_is_poorly_served_says_what_to_do_instead() {
    let poor: Vec<&str> = DEVICES
        .iter()
        .filter(|one| one.support.wants_an_alternative())
        .filter(|one| one.instead.is_none())
        .map(|one| one.device)
        .collect();

    assert!(
        poor.is_empty(),
        "these are named as poorly served and offer nothing else to try: {poor:?}"
    );
    assert!(
        DEVICES.iter().any(|one| one.support.wants_an_alternative()),
        "no device is named as poorly served, so the rule above checked nothing"
    );
}

/// Every device names something to use.
#[test]
fn every_device_says_what_to_use_on_it() {
    let silent: Vec<&str> = DEVICES
        .iter()
        .filter(|one| one.client.is_empty())
        .map(|one| one.device)
        .collect();

    assert!(silent.is_empty(), "these name no client: {silent:?}");
    assert!(
        DEVICES.len() > 5,
        "the table is too short to be a landscape"
    );
}

/// The caution names the transcode as what playback trouble is likely to be.
///
/// Naming the preset alone would leave a household to work out why a preset has
/// anything to do with a video that stops, which is the step nobody takes.
#[test]
fn a_preset_this_machine_can_only_transcode_in_software_names_the_transcode() {
    let strained = warn_before_confirming(
        Preset::Maximum,
        Playback::of(Environment::MacOs, Library::JellyfinDocker),
    );
    assert!(strained.is_some(), "the fixture must warrant a caution");

    assert_eq!(
        guidance(strained).straining,
        Some(Straining {
            preset: Preset::Maximum.label(),
            caution: PLAYBACK_WILL_STRUGGLE,
            instead: A_LIGHTER_PRESET,
        }),
        "the preset the operator chose is what the caution is about"
    );
    assert!(
        PLAYBACK_WILL_STRUGGLE.contains("transcode"),
        "the cause is not named: {PLAYBACK_WILL_STRUGGLE}"
    );
    assert!(
        PLAYBACK_WILL_STRUGGLE.contains("likely cause"),
        "the transcode is not named as the likely cause: {PLAYBACK_WILL_STRUGGLE}"
    );
    assert!(
        A_LIGHTER_PRESET.contains("lighter preset"),
        "nothing is offered that would stop it: {A_LIGHTER_PRESET}"
    );
}

/// Where nothing is strained the guidance gains no sentence.
///
/// A caution shown to everybody says nothing about anybody's machine. The three
/// ways not to warrant one are all here — a preset that provokes no transcode, a
/// host that transcodes in hardware, and no media server at all — because each
/// reaches this through a different arm of the decision.
#[test]
fn guidance_that_warrants_no_caution_does_not_gain_one() {
    let software_only = Playback::of(Environment::MacOs, Library::JellyfinDocker);
    let mut asked = 0_usize;
    for (preset, playback) in [
        (Preset::Balanced, software_only),
        (
            Preset::Maximum,
            Playback::of(Environment::LinuxNative, Library::JellyfinDocker),
        ),
        (
            Preset::Maximum,
            Playback::of(Environment::MacOs, Library::None),
        ),
    ] {
        asked += 1;
        let all = guidance(warn_before_confirming(preset, playback));
        assert!(
            all.straining.is_none(),
            "{preset:?} on {playback:?} gained a caution it does not warrant"
        );
        assert_eq!(
            all.devices.len(),
            DEVICES.len(),
            "and still answers in full"
        );
    }
    assert_eq!(asked, 3, "each way of warranting nothing is exercised");
}

/// Both statements true of every device are present.
#[test]
fn the_limits_that_hold_for_everything_are_stated() {
    let all = guidance(None);

    assert_eq!(all.devices.len(), DEVICES.len());
    assert!(
        all.only_at_home.contains("home network"),
        "the household is told where this works: {ONLY_AT_HOME}"
    );
    assert!(
        all.nothing_is_installed.contains("does not install"),
        "and what lemonfiber will not do: {NOTHING_IS_INSTALLED}"
    );
}
