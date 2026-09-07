//! What a removal would take, on a terminal.
//!
//! The list leads, because it is the whole point: an operator agreeing to a summary
//! is agreeing to something they cannot check, and the list is what they can. Each
//! line says what it is, what it occupies, and — where it is being left — why.
//!
//! What is kept comes with it rather than after it. The sentence somebody has to have
//! read before they agree is that their library survives, and putting it below the
//! list is putting it where a long list buries it.

use lemonfiber_core::bytes::humanize;
use lemonfiber_core::plural::s;
use lemonfiber_core::uninstall::{Item, Manifest, Removal, Uninstall};

use super::Lines;

/// What a removal would come to, or what it came to.
pub(crate) fn removal(report: &Uninstall) -> Lines {
    let mut lines = manifest(&report.manifest);
    lines.extend(became(&report.removal));
    lines
}

/// The list, and everything an operator reads before deciding on it.
fn manifest(manifest: &Manifest) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("Removing `{}` takes:", manifest.tier.name()));
    lines.put(format!("  {}", manifest.removes));
    lines.spaced("And leaves:");
    lines.put(format!("  {}", manifest.keeps));

    if manifest.items.is_empty() {
        lines.spaced("Nothing of this was found on this machine.");
    } else {
        lines.spaced(format!(
            "{} — {} to be got back:",
            counted(manifest),
            humanize(manifest.bytes)
        ));
        for item in &manifest.items {
            lines.extend(line(item));
        }
    }

    lines.extend(beside(manifest));
    lines.extend(waiting(manifest));
    lines.extend(unread(manifest));
    lines.extend(outside(manifest));
    lines
}

/// How many lines are going, and how many are being left.
fn counted(manifest: &Manifest) -> String {
    let going = manifest.items.iter().filter(|item| item.goes()).count();
    let kept = manifest.items.len().saturating_sub(going);
    if kept == 0 {
        return format!("{going} to remove");
    }
    format!("{going} to remove, {kept} left where they are")
}

/// One line of the list.
fn line(item: &Item) -> Lines {
    let mut lines = Lines::default();
    let size = item
        .bytes
        .map(|bytes| format!("  ({})", humanize(bytes)))
        .unwrap_or_default();
    lines.put(format!(
        "  {} {}{size}",
        if item.goes() { "-" } else { "·" },
        item.name
    ));
    lines.put(format!("      {}", item.what));
    if let Some(why) = &item.kept {
        lines.put(format!("      kept: {why}"));
    }
    if item.secret {
        lines.put("      holds a credential, which removing destroys");
    }
    lines
}

/// What is beneath the data location that the stack did not put there, and what the
/// volume is.
fn beside(manifest: &Manifest) -> Lines {
    let mut lines = Lines::default();
    if let Some(volume) = &manifest.volume {
        lines.spaced(format!("Note: {volume}."));
    }
    if manifest.foreign.is_empty() {
        return lines;
    }
    lines.spaced("Beneath the data location, and not the stack's:");
    for found in &manifest.foreign {
        lines.put(format!(
            "  {} — {} file{} ({})",
            found.at,
            found.files,
            s(usize::try_from(found.files).unwrap_or(usize::MAX)),
            humanize(found.bytes)
        ));
    }
    lines.put("  The data location is not removed as one tree while these are there.");
    lines
}

/// What is still coming down, and the backup offered before anything goes.
fn waiting(manifest: &Manifest) -> Lines {
    let mut lines = Lines::default();
    if !manifest.coming.is_empty() {
        lines.spaced("Still coming down — stopping now interrupts these:");
        for coming in &manifest.coming {
            lines.put(format!("  {} ({}%)", coming.name, coming.progress));
        }
        lines.put("  Add --wait to let them finish first.");
    }
    if let Some(backup) = &manifest.backup {
        lines.spaced(backup.clone());
    }
    lines
}

/// What this reading could not read, so a short list says why it is short.
fn unread(manifest: &Manifest) -> Lines {
    let mut lines = Lines::default();
    if manifest.confidence.complete {
        return lines;
    }
    lines.spaced("This reading is incomplete:");
    for why in &manifest.confidence.unread {
        lines.put(format!("  · {why}"));
    }
    lines
}

/// What an uninstall leaves behind, and how to remove each of them.
fn outside(manifest: &Manifest) -> Lines {
    let mut lines = Lines::default();
    if manifest.outside.is_empty() {
        return lines;
    }
    lines.spaced("Not lemonfiber's to remove, and left on this machine:");
    for entry in &manifest.outside {
        lines.put(format!(
            "  {}{}",
            entry.what,
            if entry.found { " — found here" } else { "" }
        ));
        lines.put(format!("      {}", entry.why));
        lines.put(format!("      to remove: {}", entry.by_hand));
    }
    lines
}

/// What became of the removal, where one was asked for.
fn became(removal: &Removal) -> Lines {
    let mut lines = Lines::default();
    match removal {
        Removal::Surveyed => {
            lines.spaced("Nothing was removed. Add --confirm to carry this out.");
        }
        Removal::Confirmed => {
            lines.spaced("Agreed to, and nothing was removed: this was a rehearsal.");
        }
        Removal::Complete { gone, credentials } => {
            lines.extend(went(gone, credentials));
            lines.spaced("Everything listed is gone.");
        }
        Removal::Partial {
            gone,
            credentials,
            left,
        } => {
            lines.extend(went(gone, credentials));
            lines.spaced("These could not be removed:");
            for one in left {
                lines.put(format!("  {}", one.name));
                lines.put(format!("      {}", one.why));
                lines.put(format!("      to finish: {}", one.by_hand));
            }
        }
    }
    lines
}

/// What went, and the credentials that went with it.
fn went(gone: &[String], credentials: &[String]) -> Lines {
    let mut lines = Lines::default();
    if !gone.is_empty() {
        lines.spaced("Removed:");
        for one in gone {
            lines.put(format!("  {one}"));
        }
    }
    if !credentials.is_empty() {
        lines.spaced("Credentials destroyed — these are no longer on this machine:");
        for one in credentials {
            lines.put(format!("  {one}"));
        }
    }
    lines
}
