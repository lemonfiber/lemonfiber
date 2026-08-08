//! What a capture or a restore tells the operator it did.
//!
//! Built rather than printed, like every other answer this binary gives, so what
//! it says can be read back by a test rather than only watched happening.

use lemonfiber_core::app::backup::Report as BackupReport;
use lemonfiber_core::app::restore::{Preview, Report as RestoreReport};
use lemonfiber_core::backup::Scope;

use super::UNSERIALISABLE;
use crate::render::Lines;

pub(super) fn render_backup(report: &BackupReport, json: bool) -> Lines {
    let mut lines = Lines::default();
    if json {
        // Built eagerly, like the envelope's: a report of owned values cannot fail
        // to serialise, so a lazy fallback would be a line no test could reach.
        lines.put(serde_json::to_string(report).unwrap_or(UNSERIALISABLE.to_owned()));
        return lines;
    }
    lines.put(format!(
        "Backed up {} to {}",
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
        lines.put(format!("Pruned {} older backup(s).", report.pruned.len()));
    }
    lines
}

pub(super) fn render_preview(preview: &Preview, json: bool) -> Lines {
    let mut lines = Lines::default();
    if json {
        // The preview's own type is not yet serialised; report the essentials.
        lines.put(format!(
            r#"{{"kind":"restore-preview","version":{version:?},"scope":{scope:?},"downgrade":{downgrade}}}"#,
            version = preview.manifest.product_version,
            scope = scope_name(&preview.manifest.scope),
            downgrade = preview.downgrade,
        ));
        return lines;
    }
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

pub(super) fn render_restore(report: &RestoreReport, json: bool) -> Lines {
    let mut lines = Lines::default();
    if json {
        lines.put(format!(
            r#"{{"kind":"restore","from_version":{version:?},"scope":{scope:?}}}"#,
            version = report.from_version,
            scope = scope_name(&report.scope),
        ));
        return lines;
    }
    lines.put(format!(
        "Restored {} from a backup taken by lemonfiber {}.",
        scope_name(&report.scope),
        report.from_version
    ));
    if let Some(relocation) = &report.relocated {
        lines.put(format!(
            "Re-pointed the data root from {} to {}.",
            relocation.was, relocation.now
        ));
    }
    lines
}

/// How a scope reads in a line of output.
pub(super) fn scope_name(scope: &Scope) -> String {
    match scope {
        Scope::WholeStack => "the whole stack".to_owned(),
        Scope::Service { name } => format!("service {name}"),
    }
}

/// What a restore leaves the operator to do, once the files are back in place.
pub(super) fn next_steps() -> Lines {
    let mut lines = Lines::default();
    lines.put(
        "Now bring the stack up and reconcile its wiring:  lemonfiber up <form> && lemonfiber seed",
    );
    lines.put(
        "Then check the restored credentials still work:  lemonfiber doctor --only credentials",
    );
    lines
}
