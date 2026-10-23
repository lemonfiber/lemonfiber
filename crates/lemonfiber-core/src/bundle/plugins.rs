//! What a bundle says about the plugins installed, and what it keeps of theirs.

use crate::config::PLUGIN_SETTING;
use crate::plugin::Installed;

/// Why a plugin's own configuration stays out of a bundle.
pub(crate) const WITHHELD: &str = "its own configuration was not collected, because it is the \
     plugin's and nothing here can tell which of it is safe to share";

/// Said where nothing is installed.
pub(crate) const NONE_INSTALLED: &str = "no plugin is installed";

/// Said for a plugin whose record names no source.
const UNRECORDED: &str = "a source its record does not name";

/// One line per installed plugin: its id, its version, where it came from, whether anybody
/// reviewed it, and that its own configuration was withheld.
pub(crate) fn listed(installed: &[Installed]) -> String {
    if installed.is_empty() {
        return NONE_INSTALLED.to_owned();
    }
    installed
        .iter()
        .map(|one| {
            let from = if one.from.is_empty() {
                UNRECORDED
            } else {
                one.from.as_str()
            };
            let reviewed = if one.declared.reviewed {
                "reviewed"
            } else {
                "not reviewed"
            };
            format!(
                "{} {}, from {from}, {reviewed}; {WITHHELD}",
                one.plugin, one.version
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The configuration without any setting of an installed plugin's.
pub(crate) fn withheld(configuration: &str) -> String {
    configuration
        .lines()
        .filter(|line| !line.trim_start().starts_with(PLUGIN_SETTING))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests;
