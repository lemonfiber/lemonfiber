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
mod tests;
