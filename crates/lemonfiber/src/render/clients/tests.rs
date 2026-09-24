use super::guidance;
use lemonfiber_core::clients::{Support, A_LIGHTER_PRESET, DEVICES, PLAYBACK_WILL_STRUGGLE};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::quality::Preset;
use lemonfiber_core::transcoding::{warn_before_confirming, Playback};
use lemonfiber_core::wizard::Library;

/// What playback will struggle with is said before the devices, and says what
/// the trouble will be rather than only that a preset is set.
///
/// Above the table on purpose: somebody reading this is deciding what to install,
/// and a caution met after the blocks arrives after the decision it is for.
#[test]
fn what_playback_will_struggle_with_is_said_before_any_device() {
    let strained = warn_before_confirming(
        Preset::Maximum,
        Playback::of(Environment::MacOs, Library::JellyfinDocker),
    );
    assert!(strained.is_some(), "the fixture must warrant a caution");
    let drawn = guidance(&lemonfiber_core::clients::guidance(strained)).text();

    let caution = drawn.find("Playback here is likely to struggle");
    let table = drawn.find("What to watch on");
    assert!(
        caution < table && caution.is_some(),
        "the caution must come first, and be there at all: {drawn}"
    );
    assert!(
        drawn.contains(&format!("Preset in force: {}", Preset::Maximum.label())),
        "{drawn}"
    );
    assert!(
        unwrapped(&drawn).contains(&unwrapped(PLAYBACK_WILL_STRUGGLE)),
        "the transcode is not named as the likely cause: {drawn}"
    );
    assert!(
        unwrapped(&drawn).contains(&unwrapped(A_LIGHTER_PRESET)),
        "nothing is offered that would stop it: {drawn}"
    );
}

/// A machine that warrants no caution gains no sentence, and still answers.
///
/// The whole of the requirement's restraint: an operator on a host that plays
/// this smoothly must not be told to expect trouble it will not have.
#[test]
fn a_machine_that_warrants_no_caution_is_told_nothing_about_transcoding() {
    let drawn = guidance(&lemonfiber_core::clients::guidance(None)).text();

    assert!(
        !drawn.contains("Playback here is likely to struggle"),
        "{drawn}"
    );
    assert!(!drawn.contains("Preset in force:"), "{drawn}");
    assert!(
        drawn.starts_with("What to watch on"),
        "the table still leads: {drawn}"
    );
}

/// Every symptom reaches the screen with its causes and what to do.
#[test]
fn the_report_says_what_to_do_when_it_does_not_work() {
    let drawn = guidance(&lemonfiber_core::clients::guidance(None)).text();

    for one in lemonfiber_core::clients::TROUBLE {
        assert!(drawn.contains(one.symptom), "{} is missing", one.symptom);
    }
    assert!(
        drawn.contains("Which one:"),
        "no cause says how to tell it apart"
    );
    assert!(drawn.contains("Do:"), "no cause says what to do");
}

/// Where a symptom has several causes they are numbered, and where it has one
/// they are not — a lone cause numbered `1.` reads as the first of a list the
/// reader then looks for.
#[test]
fn causes_are_numbered_only_where_there_is_more_than_one() {
    let drawn = guidance(&lemonfiber_core::clients::guidance(None)).text();

    let numbered: Vec<&str> = lemonfiber_core::clients::TROUBLE
        .iter()
        .filter(|one| one.causes.len() > 1)
        .filter_map(|one| one.causes.first())
        .map(|cause| cause.because)
        .collect();
    let alone: Vec<&str> = lemonfiber_core::clients::TROUBLE
        .iter()
        .filter(|one| one.causes.len() == 1)
        .filter_map(|one| one.causes.first())
        .map(|cause| cause.because)
        .collect();

    assert!(!numbered.is_empty(), "no symptom has causes to number");
    assert!(
        !alone.is_empty(),
        "no symptom has a single cause to leave unnumbered"
    );

    let missing: Vec<&&str> = numbered
        .iter()
        .filter(|because| !drawn.contains(&format!("1. {because}")))
        .collect();
    assert!(missing.is_empty(), "these should be numbered: {missing:?}");

    let wrongly: Vec<&&str> = alone
        .iter()
        .filter(|because| drawn.contains(&format!("1. {because}")))
        .collect();
    assert!(wrongly.is_empty(), "a lone cause was numbered: {wrongly:?}");
}

/// Every device reaches the screen, and each carries its client.
#[test]
fn every_device_reaches_the_screen_with_something_to_use() {
    let drawn = guidance(&lemonfiber_core::clients::guidance(None)).text();

    for device in DEVICES {
        assert!(
            drawn.contains(device.device),
            "{} is missing from the report",
            device.device
        );
        assert!(
            drawn.contains(device.client),
            "{} names no client on screen",
            device.device
        );
    }
}

/// A poorly-served device is named as one and carries its alternative.
///
/// The last assertion holds the corpus: without it this passes on a table where
/// nothing is marked poorly served.
#[test]
fn a_hard_case_is_named_as_one_and_offers_a_way_out() {
    let drawn = guidance(&lemonfiber_core::clients::guidance(None)).text();

    assert!(drawn.contains("poorly served"), "{drawn}");
    assert!(
        drawn.contains("Better:"),
        "a device named as poorly served offers nothing else: {drawn}"
    );
    assert!(
        DEVICES.iter().any(|one| one.support == Support::Poor),
        "no device is poorly served, so this checked nothing"
    );
}

/// What holds for every device is written once, not per device.
///
/// Matched with the line breaks taken out: both are wrapped on the way to the
/// screen, so the words arrive split across lines the source does not have.
#[test]
fn the_limits_are_said_once_rather_than_per_device() {
    let all = lemonfiber_core::clients::guidance(None);
    let drawn = unwrapped(&guidance(&all).text());

    assert_eq!(
        drawn.matches(&unwrapped(all.only_at_home)).count(),
        1,
        "the home-network limit is repeated: {drawn}"
    );
    assert_eq!(
        drawn.matches(&unwrapped(all.nothing_is_installed)).count(),
        1,
        "what lemonfiber will not do is repeated: {drawn}"
    );
}

/// The text with every run of whitespace made one space.
fn unwrapped(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}
