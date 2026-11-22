//! What the plugin catalogue offers, on a terminal.

use lemonfiber_core::model::{CataloguedPlugin, PluginCatalogueReport, PluginCatalogueState};

use super::super::Lines;

/// What the listing is, said before it.
const HEADING: &str = "What the plugin catalogue offers to install:";

/// What the catalogue offers, or why it was not read.
pub(crate) fn listed(report: &PluginCatalogueReport) -> Lines {
    let mut lines = Lines::default();
    match report.state {
        PluginCatalogueState::Off => {
            lines.put("Asking the plugin catalogue is switched off, so it was not read.");
            if let Some(setting) = &report.setting {
                lines.put(format!(
                    "  Allow it with:  lemonfiber config set {} {}",
                    setting.key, setting.value
                ));
            }
        }
        PluginCatalogueState::Failed => {
            lines.put("The plugin catalogue could not be read, so nothing is listed.");
            if let Some(failure) = &report.failure {
                lines.put(format!("  {} ({})", failure.summary, failure.code));
            }
        }
        PluginCatalogueState::Listed => {
            lines.put(HEADING);
            if report.entries.is_empty() {
                lines.put("  The catalogue offers no plugin this build can list.");
            }
            for entry in &report.entries {
                lines.extend(entry_of(entry));
            }
            if let Some(skipped) = report.unreadable.filter(|count| *count > 0) {
                lines.spaced(format!(
                    "{skipped} more could not be listed; a newer lemonfiber may read them."
                ));
            }
            if let (Some(published), Some(serial)) = (report.published, report.serial) {
                lines.spaced(format!(
                    "Release {serial} of the catalogue, published {}.",
                    published.said()
                ));
            }
        }
    }
    lines
}

/// One plugin: its id and name, what it does, what going without it costs, its version,
/// and what is installed of it.
fn entry_of(entry: &CataloguedPlugin) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(format!("  {} — {}", entry.id, entry.name));
    lines.put(format!("    does       {}", entry.description));
    lines.put(format!("    without    {}", entry.without_it));
    lines.put(format!("    version    {}", entry.version));
    let installed = entry.installed.as_ref().map_or_else(
        || "no".to_owned(),
        |held| {
            if held.newer {
                format!("{}, and the catalogue holds a newer one", held.version)
            } else {
                held.version.clone()
            }
        },
    );
    lines.put(format!("    installed  {installed}"));
    lines
}

#[cfg(test)]
mod tests;
