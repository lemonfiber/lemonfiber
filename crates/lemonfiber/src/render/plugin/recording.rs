//! What taking a plugin's recordings again came to, on a terminal.

use lemonfiber_core::plugin::{Recorded, Took};

use super::super::Lines;
use super::{counted, document};

/// What taking a plugin's recordings again came to, in whichever form was asked.
pub(crate) fn recorded(read: &Recorded, json: bool) -> Option<Lines> {
    if json {
        return serde_json::to_string_pretty(read)
            .ok()
            .as_deref()
            .map(document);
    }
    Some(said(read))
}

/// Each recording and what taking it came to, each service that failed, and what the
/// whole comes to.
fn said(read: &Recorded) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{} — every recording its manifest names, taken again from its pin",
        read.id
    ));
    if !read.refusals.is_empty() {
        lines.spaced(format!(
            "Refused, {}, so nothing was taken:",
            counted(read.refusals.len(), "violation")
        ));
        for refusal in &read.refusals {
            lines.put(format!("  {} — {}", refusal.location, refusal.message));
        }
    }
    for one in &read.recordings {
        let fixture = one.fixture.as_deref().unwrap_or("no recording");
        lines.put(format!(
            "  {:<34} {fixture}: {}",
            one.named,
            took(&one.took)
        ));
    }
    for failed in &read.failed {
        lines.spaced(format!(
            "{} failed at {}: {}. None of its recordings was written.",
            failed.service, failed.step, failed.why
        ));
        if !failed.log.is_empty() {
            lines.put(format!("  The last {} lines it wrote:", failed.log.len()));
            for line in &failed.log {
                lines.put(format!("    {line}"));
            }
        }
    }
    lines.spaced(if read.holds && read.recordings.is_empty() {
        "It names no recording, so there is nothing to take."
    } else if read.holds {
        "Every recording is taken."
    } else {
        "Not every recording is taken."
    });
    lines
}

/// What taking one recording came to, in a few words.
fn took(took: &Took) -> String {
    match took {
        Took::Written => "written".to_owned(),
        Took::Unnoted => {
            "written with an empty note, which has to say why it is evidence".to_owned()
        }
        Took::Reproduced => "reproduced, and left as it was".to_owned(),
        Took::Unreproduced { why } => format!("refused: {why}"),
        Took::Overran { seconds } => {
            format!("still running at its limit of {seconds} seconds, and stopped")
        }
        Took::Unrecorded { why } => format!("not recorded: {why}"),
    }
}

#[cfg(test)]
mod tests;
