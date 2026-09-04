//! Which app to watch on, drawn for a terminal.
//!
//! One device per block: the device and its standing on a line of its own, what to
//! use and any caution indented under it, wrapped at [`WIDTH`]. The two statements
//! true of every device are written once, after the blocks.
//!
//! Where playback here will struggle whatever is installed, that goes first — above
//! the table rather than under a device, because it is settled before anybody
//! chooses one and a reader who meets it after the blocks has already decided.

use lemonfiber_core::clients::{Device, Guidance, Straining, Support, Trouble};
use lemonfiber_core::text::Overrun;

use super::Lines;

/// How far the detail under a device is indented.
const UNDER: &str = "    ";

/// How far a wrapped continuation is indented, past the detail it continues.
const WRAPPED: &str = "      ";

/// Where a line is broken.
const WIDTH: usize = 76;

/// Which app to use, device by device.
pub(crate) fn guidance(all: &Guidance) -> Lines {
    let mut lines = Lines::default();
    if let Some(straining) = all.straining {
        strained(&mut lines, straining);
    }
    lines.put("What to watch on, and what to use:");
    for device in &all.devices {
        entry(&mut lines, device);
    }
    lines.spaced("When it does not work:");
    for one in &all.trouble {
        symptom(&mut lines, one);
    }
    closing(&mut lines, all.only_at_home);
    closing(&mut lines, all.nothing_is_installed);
    lines
}

/// What playback here will struggle with, above the table and in the table's own
/// shape: a heading of its own, the detail indented under it, and the way out on the
/// `Better:` line a poorly-served device uses.
///
/// Ends on a blank line, so the devices below start where they start when there is
/// no caution at all.
fn strained(lines: &mut Lines, straining: Straining) {
    lines.put("Playback here is likely to struggle, whatever app is used");
    detail(lines, &format!("Preset in force: {}", straining.preset));
    detail(lines, straining.caution);
    detail(lines, &format!("Better: {}", straining.instead));
    lines.put(String::new());
}

/// One symptom, and each thing that could be behind it.
///
/// The symptom on its own line and the causes under it, numbered where there is
/// more than one — a reader working through three possibilities needs to know which
/// they are on.
fn symptom(lines: &mut Lines, one: &Trouble) {
    lines.spaced(one.symptom.to_owned());
    let many = one.causes.len() > 1;
    for (at, cause) in one.causes.iter().enumerate() {
        let led = if many {
            format!("{}. {}", at + 1, cause.because)
        } else {
            cause.because.to_owned()
        };
        detail(lines, &led);
        detail(lines, &format!("Which one: {}", cause.tell));
        detail(lines, &format!("Do: {}", cause.fix));
    }
}

/// One device, what to use on it, and what is worth knowing before starting.
fn entry(lines: &mut Lines, device: &Device) {
    lines.spaced(format!("{} — {}", device.device, standing(device.support)));
    detail(lines, &format!("Use: {}", device.client));
    if let Some(caution) = device.caution {
        detail(lines, caution);
    }
    if let Some(instead) = device.instead {
        detail(lines, &format!("Better: {instead}"));
    }
}

/// A statement true of every device, wrapped and unindented, after a blank line.
fn closing(lines: &mut Lines, text: &str) {
    let mut first = true;
    for line in lemonfiber_core::text::wrapped(text, WIDTH, Overrun::Allowed) {
        if first {
            lines.spaced(line);
            first = false;
        } else {
            lines.put(line);
        }
    }
}

/// A sentence indented under the device it is about, wrapped to the width.
fn detail(lines: &mut Lines, text: &str) {
    let mut indent = UNDER;
    for line in lemonfiber_core::text::wrapped(text, WIDTH, Overrun::Allowed) {
        lines.put(format!("{indent}{line}"));
        indent = WRAPPED;
    }
}

/// How well served a device is, as the report words it.
const fn standing(support: Support) -> &'static str {
    match support {
        Support::Good => "works well",
        Support::Workable => "works, with something to know",
        Support::Poor => "poorly served",
        Support::Fallback => "always works",
    }
}

#[cfg(test)]
mod tests {
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
}
