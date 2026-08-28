//! Which app to watch on, drawn for a terminal.
//!
//! One device per block: the device and its standing on a line of its own, what to
//! use and any caution indented under it, wrapped at [`WIDTH`]. The two statements
//! true of every device are written once, after the blocks.

use lemonfiber_core::clients::{Device, Guidance, Support, Trouble};
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
    use lemonfiber_core::clients::{Support, DEVICES};

    /// Every symptom reaches the screen with its causes and what to do.
    #[test]
    fn the_report_says_what_to_do_when_it_does_not_work() {
        let drawn = guidance(&lemonfiber_core::clients::guidance()).text();

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
        let drawn = guidance(&lemonfiber_core::clients::guidance()).text();

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
        let drawn = guidance(&lemonfiber_core::clients::guidance()).text();

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
        let drawn = guidance(&lemonfiber_core::clients::guidance()).text();

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
        let all = lemonfiber_core::clients::guidance();
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
