//! A substitute standing in for the bundle's choice of a capability, as an operator puts
//! one in its place: installed from what this build offers, chosen to fill the capability,
//! and given the settings it is asked with.

use super::core::{Core, Ran, WITHHELD};

/// What every download capability is named under.
const DOWNLOADS: &str = "download.";

/// Why the arm chooses the substitute, as the choice records it.
const BECAUSE: &str = "the substitutes arm proves it stands in";

/// A substitute put in place of the bundle's choice, debugged with each setting's value
/// withheld.
#[derive(Clone)]
pub struct Substitution {
    /// The substitute's `[plugin].id`.
    pub plugin: String,
    /// The capability it stands in for.
    pub capability: String,
    /// The plugin the bundle fills the capability with, which it replaces.
    pub stands_in_for: String,
    /// Each setting it is given, as `config set` names it, with its value.
    pub settings: Vec<(String, String)>,
}

impl std::fmt::Debug for Substitution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let settings: Vec<(&str, &str)> = self
            .settings
            .iter()
            .map(|(setting, _)| (setting.as_str(), WITHHELD))
            .collect();
        f.debug_struct("Substitution")
            .field("plugin", &self.plugin)
            .field("capability", &self.capability)
            .field("stands_in_for", &self.stands_in_for)
            .field("settings", &settings)
            .finish()
    }
}

impl Substitution {
    /// Whether it stands in for a download client.
    #[must_use]
    pub fn downloads(&self) -> bool {
        self.capability.starts_with(DOWNLOADS)
    }

    /// Every service of the part it stands in for, which its choice stops.
    #[must_use]
    pub fn passed_over(&self) -> Vec<String> {
        super::services_of(&self.stands_in_for)
            .iter()
            .filter_map(|service| service.get("id").and_then(toml::Value::as_str))
            .map(str::to_owned)
            .collect()
    }

    /// Install it, choose it, and give it its settings, each step held to succeeding.
    ///
    /// # Errors
    ///
    /// The first step that did not, with what was said.
    pub fn made(&self, core: &Core) -> Result<(), String> {
        held(
            &core.agreed(&["plugin", "install", &self.plugin]),
            "install",
        )?;
        let service = super::providing(&self.plugin, &self.capability)
            .ok_or_else(|| format!("no service of {} provides {}", self.plugin, self.capability))?;
        held(
            &core.agreed(&[
                "wiring",
                "fill",
                &self.capability,
                &service,
                "--reason",
                BECAUSE,
            ]),
            "the choice",
        )?;
        for (setting, value) in &self.settings {
            held(
                &core.withholding(
                    &["config", "set", setting, value, "--confirm"],
                    "",
                    &[value],
                ),
                setting,
            )?;
        }
        Ok(())
    }
}

/// `ran`, where it succeeded, or `what` and what it said.
fn held(ran: &Ran, what: &str) -> Result<(), String> {
    if ran.succeeded {
        Ok(())
    } else {
        Err(format!("{what} failed:\n{}", ran.printed))
    }
}
