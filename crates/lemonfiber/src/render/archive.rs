//! What a capture, a restore and a support bundle tell the operator they did.
//!
//! The three answers about an archive, together because they share the sentence
//! that names what one covers and because all three end by telling somebody what
//! is now on their disk and how careful to be with it. The listing of what has been
//! kept is here for the same reason: it is what a restore is asked with, and it
//! ends by saying how to ask.
//!
//! A restore's listing and what a restore did are one answer with the listing
//! always in it, and only one of the two is worth reading at a time: before, the
//! listing is the whole point and there is nothing else to say; after, repeating it
//! would put the same paragraph on the screen twice in one run.

use std::path::Path;

use lemonfiber_core::app::archives::Listing;
use lemonfiber_core::app::restore::{Preview, Restoration};
use lemonfiber_core::app::support::Bundle;
use lemonfiber_core::backup::run::Report as Capture;
use lemonfiber_core::backup::Scope;
use lemonfiber_core::bytes::humanize;

use super::Lines;

/// Where a backup went, and how private it is — or where one would go.
///
/// One rendering in two tenses rather than two renderings, because a capture is
/// settled before it is written: the destination, what it would hold, how large it is
/// and which older archives retention has no room for are all worked out before the
/// archive exists. Only the verbs move.
pub(crate) fn backup(report: &Capture) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{} {} to {}",
        if report.rehearsed {
            "Would back up"
        } else {
            "Backed up"
        },
        scope_name(&report.scope),
        report.path.display()
    ));
    if report.sensitive {
        lines.put(
            "This backup contains credentials — the VPN key, provider passwords and API keys. \
             Keep it as private as the secrets inside it.",
        );
    }
    if !report.pruned.is_empty() {
        lines.put(format!(
            "{} {} older backup(s): {}.",
            if report.rehearsed {
                "Would prune"
            } else {
                "Pruned"
            },
            report.pruned.len(),
            report.pruned.join(", ")
        ));
    }
    // Said only where there is something to say. A capture inside the budget is done
    // while somebody is still reading the line above it, and announcing that on every
    // backup anybody ever takes would be noise. A capture past it is the one an
    // operator wonders about afterwards, and the answer is its size.
    if !report.pace.brisk {
        lines.put(format!(
            "This moved {} — past the {} a capture is reckoned to manage in a minute, so a \
             wait here is the size of what you keep rather than a fault.",
            humanize(report.pace.moved),
            humanize(report.pace.budget)
        ));
    }
    if report.rehearsed {
        lines.spaced("Nothing has been written. Run it without --dry-run to take the backup.");
    }
    lines
}

/// Which backups this machine has kept, and how to put one back.
///
/// The names and nothing else, because a name is what a restore is asked for and
/// what an archive holds is the answer to naming one. Newest first, since the
/// archive most often wanted is the one taken last.
pub(crate) fn kept(listing: &Listing) -> Lines {
    let mut lines = Lines::default();
    if listing.archives.is_empty() {
        lines.put("No backups have been taken on this machine yet.");
        lines.spaced("Take one with:  lemonfiber backup");
        return lines;
    }
    lines.put("Backups kept on this machine, newest first:");
    for name in &listing.archives {
        lines.put(format!("  {name}"));
    }
    lines.spaced("Put one back with:  lemonfiber restore <archive>");
    lines
}

/// What a restore would overwrite, or what it put back.
pub(crate) fn restoration(report: &Restoration) -> Lines {
    match &report.done {
        None => would(&report.would),
        Some(done) => {
            let mut lines = Lines::default();
            lines.put(format!(
                "Restored {} from a backup taken by lemonfiber {}.",
                scope_name(&done.scope),
                done.from_version
            ));
            if let Some(relocation) = &done.relocated {
                lines.put(format!(
                    "Re-pointed the data root from {} to {}.",
                    relocation.was, relocation.now
                ));
            }
            lines.extend(next_steps());
            lines
        }
    }
}

/// The archive's own account of itself, read before anything is overwritten.
fn would(preview: &Preview) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "This backup holds {}, taken by lemonfiber {} on {}.",
        scope_name(&preview.manifest.scope),
        preview.manifest.product_version,
        preview.manifest.created_at,
    ));
    for member in &preview.manifest.members {
        lines.put(format!("  - {}", member.label));
    }
    if preview.downgrade {
        lines.put(
            "It is from an older major version; restoring it is allowed but may need a further \
             reconcile.",
        );
    }
    if let Some(relocation) = &preview.relocation {
        lines.put(format!(
            "It was taken against a different data root ({} → {}); re-run with --repoint to \
             restore onto this machine's.",
            relocation.was, relocation.now
        ));
    }
    lines
}

/// What a bundle would hold, or where the one that exists went.
pub(crate) fn bundle(report: &Bundle) -> Lines {
    match &report.path {
        None => described(report),
        Some(path) => written(report, path),
    }
}

/// What a bundle would hold, while there is still nothing to attach.
///
/// The size is stated here rather than after writing, which is the whole reason a bare run
/// writes nothing: an operator decides whether to make this file at the one moment the
/// answer can still change what they do.
fn described(report: &Bundle) -> Lines {
    let contents = &report.contents;
    let mut lines = Lines::default();
    lines.put("A support bundle would hold:");
    for (name, _) in contents.files() {
        lines.put(format!("  {name}"));
    }
    lines.spaced(format!("{} in all.", humanize(report.bytes)));
    // Named rather than passed over, and set apart rather than mixed into the listing: a
    // gap nobody mentions reads as an absence of trouble instead of an absence of
    // information, and one buried among the filenames reads as neither.
    if !contents.missing.is_empty() {
        lines.spaced("Could not be read:");
        for gap in &contents.missing {
            lines.put(format!("  {gap}"));
        }
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
    // Where it would land, said with the sentence that says it has not been written.
    // A description of what would be in the file and not of where the file would be is
    // half an answer: an operator deciding at a shell wants to know what they would
    // then have to go and find.
    if let Some(at) = &report.would_go {
        lines.spaced(format!("It would be written to {}.", at.display()));
    }
    lines.spaced("Nothing has been written. Run `lemonfiber support --write` to produce it.");
    lines
}

/// Where a bundle went, how large it is, and what a reader will find in it.
fn written(report: &Bundle, path: &Path) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "Written to {} ({})",
        path.display(),
        humanize(report.bytes)
    ));
    for (name, _) in report.contents.files() {
        lines.put(format!("  {name}"));
    }
    lines
        .spaced("Nothing has left this machine. Read it before you send it, and send it yourself.");
    lines
}

/// How a scope reads in a line of output.
fn scope_name(scope: &Scope) -> String {
    match scope {
        Scope::WholeStack => "the whole stack".to_owned(),
        Scope::Service { name } => format!("service {name}"),
        // Named by the setup rather than by the trees, because the trees are listed
        // underneath it and the project is what the operator recognises it by.
        Scope::Existing { project, .. } => format!("the setup {project}, taken over"),
    }
}

/// What a restore leaves the operator to do, once the files are back in place.
fn next_steps() -> Lines {
    let mut lines = Lines::default();
    lines.put(
        "Now bring the stack up and reconcile its wiring:  lemonfiber up <form> && lemonfiber seed",
    );
    lines.put(
        "Then check the restored credentials still work:  lemonfiber doctor --only credentials",
    );
    lines
}

#[cfg(test)]
mod tests;
