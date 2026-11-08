//! A substitute standing in for the bundle's choice of a capability, as an operator puts
//! one in its place: installed from what this build offers, chosen to fill the capability,
//! and given the settings it is asked with.

use super::core::{Core, Ran};

/// Why the arm chooses the substitute, as the choice records it.
const BECAUSE: &str = "the substitutes arm proves it stands in";

/// A substitute put in place of the bundle's choice.
#[derive(Debug, Clone)]
pub struct Substitution {
    /// The substitute's `[plugin].id`.
    pub plugin: String,
    /// The capability it stands in for.
    pub capability: String,
    /// Each setting it is given, as `config set` names it, with its value.
    pub settings: Vec<(String, String)>,
}

impl Substitution {
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
                &core.run(&["config", "set", setting, value, "--confirm"], ""),
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
