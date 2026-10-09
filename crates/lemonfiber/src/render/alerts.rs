//! What the operator is told about, printed.

use lemonfiber_core::model::AlertReport;

use super::Lines;

/// What the operator is told about, what that means, and anything set apart from it.
pub(super) fn alerts(report: &AlertReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("telling you about: {}", report.preset));
    lines.put(report.means.clone());
    for exception in &report.exceptions {
        // Named apart from the preset, so the operator can see why one kind does not
        // follow the answer they just read.
        lines.put(format!(
            "  {} — {}",
            exception.kind,
            if exception.wanted {
                "always told"
            } else {
                "never told"
            }
        ));
    }
    if report.changed {
        lines.put(String::new());
        // A rehearsal reports what it would do, so it must not claim it saved.
        lines.put(if report.rehearsed {
            "would save"
        } else {
            "saved"
        });
    }
    lines
}
