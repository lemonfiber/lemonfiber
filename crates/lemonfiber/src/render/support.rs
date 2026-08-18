//! What a support bundle says about itself, before and after it exists.
//!
//! Both answers list what it holds, because that listing is the point: a bundle nobody
//! reads before sending is the screenshot of a config file this whole feature replaces.

use lemonfiber_core::app::bundle::Written;
use lemonfiber_core::bundle::Contents;
use lemonfiber_core::bytes::humanize;

use super::{Lines, UNRENDERABLE};

/// What a bundle would hold, while there is still nothing to attach.
///
/// The size is stated here rather than after writing, which is the whole reason a bare run
/// writes nothing: an operator decides whether to make this file at the one moment the
/// answer can still change what they do.
pub(crate) fn render_preview(contents: &Contents, bytes: u64, json: bool) -> Lines {
    let mut lines = Lines::default();
    if json {
        lines.put(serde_json::to_string(contents).unwrap_or(UNRENDERABLE.to_owned()));
        return lines;
    }
    lines.put("A support bundle would hold:");
    for (name, _) in contents.files() {
        lines.put(format!("  {name}"));
    }
    lines.put(format!("  {} in all", humanize(bytes)));
    for gap in &contents.missing {
        lines.put(format!("  could not be read: {gap}"));
    }
    if !contents.terms.revealed.is_empty() {
        let (subject, verb) = if contents.terms.revealed.len() == 1 {
            ("it", "is")
        } else {
            ("they", "are")
        };
        lines.spaced(format!(
            "It will hold {} as {subject} {verb}, because you asked, and will say so on its first page.",
            contents.terms.revealed.join(", "),
        ));
    }
    lines.spaced("Nothing has been written. Run `lemonfiber support --write` to produce it.");
    lines
}

/// Where a bundle went, how large it is, and what a reader will find in it.
pub(crate) fn render_written(written: &Written, json: bool) -> Lines {
    let mut lines = Lines::default();
    if json {
        lines.put(serde_json::to_string(written).unwrap_or(UNRENDERABLE.to_owned()));
        return lines;
    }
    lines.put(format!(
        "Written to {} ({})",
        written.path.display(),
        humanize(written.bytes)
    ));
    for name in &written.holds {
        lines.put(format!("  {name}"));
    }
    lines
        .spaced("Nothing has left this machine. Read it before you send it, and send it yourself.");
    lines
}
