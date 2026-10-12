//! What recording a plugin's run proofs came to, on a terminal.

use lemonfiber_core::plugin::{Recorded, Took};

use super::super::Lines;
use super::{counted, document};

/// What recording a plugin's run proofs came to, in whichever form was asked.
pub(crate) fn recorded(read: &Recorded, json: bool) -> Option<Lines> {
    if json {
        return serde_json::to_string_pretty(read)
            .ok()
            .as_deref()
            .map(document);
    }
    Some(said(read))
}

/// Each run proof and what recording it came to, and what the whole comes to.
fn said(read: &Recorded) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{} — its run proofs, recorded by running them",
        read.id
    ));
    if !read.refusals.is_empty() {
        lines.spaced(format!(
            "Refused, {}, so nothing was run:",
            counted(read.refusals.len(), "violation")
        ));
        for refusal in &read.refusals {
            lines.put(format!("  {} — {}", refusal.location, refusal.message));
        }
    }
    for one in &read.runs {
        let fixture = one.fixture.as_deref().unwrap_or("no recording");
        lines.put(format!(
            "  {:<26} {fixture}: {}",
            one.proof,
            took(&one.took)
        ));
    }
    lines.spaced(if read.holds && read.runs.is_empty() {
        "It declares no proof that runs its image's own program, so there is nothing to record."
    } else if read.holds {
        "Every run proof is recorded."
    } else {
        "Not every run proof is recorded; a recording a run does not reproduce is refused."
    });
    lines
}

/// What recording one proof came to, in a few words.
fn took(took: &Took) -> String {
    match took {
        Took::Written => "written".to_owned(),
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
