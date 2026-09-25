//! The quality choice, what it costs, and what became of applying it.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.

use lemonfiber_core::model::{
    Disposition, MusicChoice, MusicReport, PresetChoice, QualityReport, Triggered, UpgradeReport,
};
use lemonfiber_core::PRODUCT;

use super::Lines;

/// The quality choice, what each preset means, and what the command did with it.
pub(super) fn quality(report: &QualityReport) -> Lines {
    let mut lines = Lines::default();
    for choice in &report.choices {
        lines.extend(preset_choice(choice));
    }
    if let Some(choice) = &report.music {
        lines.extend(music_choice(choice));
    }
    match report.disposition {
        // A change is forward-looking, and this is where the operator is told so —
        // the expectation is often the opposite, that lowering quality shrinks the
        // library or raising it re-grabs everything.
        Disposition::Recorded => {
            lines.spaced("Saved. This affects future acquisitions only — nothing already downloaded changes.");
            if report.customised {
                lines.put(format!(
                    "Your Recyclarr config is customised, so this preset will not apply on its \
                     own. Run `{PRODUCT} quality reapply` to let it overwrite your edits."
                ));
            }
        }
        Disposition::Rehearsed => {
            lines.spaced(
                "Would save. This affects future acquisitions only — nothing downloaded changes.",
            );
        }
        Disposition::Held => {
            lines.spaced(
                "Not saved: this machine would have to transcode this in software, which will not \
                 play well. Re-run with --confirm to choose it anyway, or run Jellyfin natively.",
            );
        }
        // Re-asserting the preset over the config: say whether it overwrote an edit.
        Disposition::Reapplied => {
            if report.customised {
                lines.spaced("Reapplied the preset, overwriting your customised Recyclarr config.");
            } else {
                lines.spaced("Reapplied the preset. The Recyclarr config was already in step.");
            }
        }
        // A rehearsed reapply: preview whether it would overwrite an edit.
        Disposition::WouldReapply => {
            if report.customised {
                lines.spaced(
                    "Would reapply the preset, overwriting your customised Recyclarr config.",
                );
            } else {
                lines.spaced("Would reapply the preset. The Recyclarr config is already in step.");
            }
        }
        // A plain show reports the state; a customised config is worth naming.
        Disposition::Shown => {
            if report.customised {
                lines.spaced(format!(
                    "Your Recyclarr config is customised — the preset is no longer authoritative. \
                     Run `{PRODUCT} quality reapply` to re-assert it over your edits."
                ));
            }
        }
    }
    // After the sentence, never instead of it, and for both reapplies: consent to an
    // overwrite is consent to something the operator was shown, and a rehearsal is
    // where they are shown it while it is still a question. Empty everywhere else,
    // because nothing but a reapply carries an edit to report.
    lines.extend(replaced(report));
    lines
}

/// The lines a reapply replaced — or, rehearsed, would replace — under the sentence
/// saying so.
///
/// Absent where the report carries none, which is a reapply over a config already in
/// lemonfiber's own hand. The lines are the ones the core masked; nothing here
/// unmasks them, and nothing here decides what a credential is.
fn replaced(report: &QualityReport) -> Lines {
    let mut lines = Lines::default();
    let Some(edit) = &report.overwritten else {
        return lines;
    };
    lines.spaced(format!("What went, in {}:", edit.path));
    for line in edit.diff.lines() {
        lines.put(format!("  {line}"));
    }
    lines
}

/// One preset in force: what it applies to, what it means, and what it costs.
pub(super) fn preset_choice(choice: &PresetChoice) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{}: {} — {}",
        choice.scope, choice.preset, choice.means
    ));
    lines.put(format!(
        "  {} · {} · {}",
        choice.resolution, choice.size_per_hour, choice.transcoding
    ));
    if choice.needs_transcoding_here {
        lines.put("  ⚠ this machine would have to transcode this in software");
    }
    lines
}

/// One audio-format choice, in the same shape as a preset choice but in format terms —
/// what it targets, its size, and the caveat worth knowing rather than a resolution.
pub(super) fn music_choice(choice: &MusicChoice) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{}: {} — {}",
        choice.scope, choice.format, choice.means
    ));
    lines.put(format!(
        "  {} · {} · {}",
        choice.targets, choice.size_per_hour, choice.note
    ));
    lines
}

/// Choosing the audio format for music: the choice, then whether it was recorded or
/// rehearsed and what became of applying it to the music service.
pub(super) fn music(report: &MusicReport) -> Lines {
    let mut lines = music_choice(&report.choice);
    if matches!(report.disposition, Disposition::Rehearsed) {
        lines.spaced(
            "Would save. This affects future acquisitions only — nothing downloaded changes.",
        );
        return lines;
    }
    lines.spaced(
        "Saved. This affects future acquisitions only — nothing already downloaded changes.",
    );
    match &report.outcome {
        None | Some(Triggered::Started) => {
            lines.put("Applied to the music service.");
        }
        Some(Triggered::NotStarted) => {
            lines.put("The music service is not up yet, so it was recorded but not applied — run this again once it is.");
        }
        Some(Triggered::Failed { detail }) => {
            lines.put(format!(
                "Recorded, but the music service refused the change: {detail}"
            ));
        }
    }
    lines
}

/// Upgrading existing content: the cost stated per media type first, then — once
/// confirmed — what each service was asked to do.
pub(super) fn upgrade(report: &UpgradeReport) -> Lines {
    let mut lines = Lines::default();
    if report.media.is_empty() {
        lines.put("No television or film service is set up, so there is nothing to upgrade.");
        return lines;
    }
    if report.confirmed {
        lines.put(
            "Upgrading existing content, each service re-searching against its own quality bar:",
        );
    } else {
        // The cost, and nothing done: a large operation stays behind a deliberate
        // confirmation.
        lines.put(
            "Upgrading existing content re-downloads your library at the chosen quality — a large, \
             bandwidth-expensive operation, potentially terabytes and hours to days. It would cost, \
             per media:",
        );
    }
    for media in &report.media {
        lines.put(format!(
            "  {}: {} — {}",
            media.media_type, media.preset, media.size_per_hour
        ));
        match &media.outcome {
            None => {}
            Some(Triggered::Started) => lines.put("    ✓ re-search started"),
            Some(Triggered::NotStarted) => {
                lines.put(
                    "    · not started — the service is not up yet; run this again once it is",
                );
            }
            Some(Triggered::Failed { detail }) => lines.put(format!("    ✗ {detail}")),
        }
    }
    if !report.confirmed {
        lines.spaced("Nothing has been changed. Re-run with --confirm to go ahead.");
    }
    lines
}

#[cfg(test)]
mod tests;
