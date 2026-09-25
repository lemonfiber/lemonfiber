//! What a plugin said about itself when it was installed, kept for as long as it is.
//!
//! Beside what the install decided rather than inside it, because the two are
//! different kinds of fact: where a container is published is a decision this build
//! took, and which hosts a plugin says it will reach is a declaration the plugin made
//! and this build held it to. Both are read off the manifest once, at install, and kept
//! here — because the author's directory may be gone the moment the install is done,
//! and the one read of what each plugin is doing has to answer about this machine
//! rather than about a document.
//!
//! **Every field is defaulted**, so a record written before these were kept reads as a
//! plugin that declared nothing, which is the answer that invents nothing — and says
//! so, rather than being refused.

use lemonfiber_plugin::Manifest;
use serde::{Deserialize, Serialize};

use super::stating::Overriding;

/// One credential a plugin says it will hold, without a value and with no place for one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginSecret")]
pub struct Secret {
    /// What the value is, within the plugin.
    pub id: String,
    /// Whose credential it is.
    pub of: String,
    /// What holding it is for.
    pub why: String,
}

/// What a plugin declared about itself, as its install read it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginDeclaration")]
pub struct Declaration {
    /// Where its source is published, as the plugin names it.
    #[serde(default)]
    pub upstream: String,
    /// The licence it is distributed under.
    #[serde(default)]
    pub license: String,
    /// Whether anybody reviewed it before it was installed.
    ///
    /// False for every install from a path an operator named — which is every install
    /// this build makes. Carried rather than left implicit, because an unreviewed
    /// plugin is to be said to be one for as long as it is installed, and a field that
    /// is only ever false today is still the field a reviewed install will set.
    #[serde(default)]
    pub reviewed: bool,
    /// Every capability it claims, core and its own, in the order it declares them.
    ///
    /// Apart from what its services fill: a claim is what it says it can do and has to
    /// demonstrate, and a capability of its own is claimed without anything asking for
    /// it.
    #[serde(default)]
    pub claims: Vec<String>,
    /// Every bundled setting it declares it may change.
    #[serde(default)]
    pub overrides: Vec<Overriding>,
    /// Every destination a recipe of its could reach that is not one of its own
    /// services: a service of this stack's, or a name outside it.
    ///
    /// Recorded as declared rather than sorted into the two here, because which names
    /// are this stack's is a question about the stack, and the stack a record is read
    /// against is the one on the machine when it is read.
    #[serde(default)]
    pub reaches: Vec<String>,
    /// Every credential it says it will hold.
    #[serde(default)]
    pub secrets: Vec<Secret>,
}

impl Declaration {
    /// What this manifest declares about its plugin.
    #[must_use]
    pub fn of(manifest: &Manifest) -> Self {
        let own = |to: &str| manifest.services.iter().any(|service| service.id == to);
        let mut reaches: Vec<String> = Vec::new();
        for to in manifest.recipes.iter().flat_map(|recipe| {
            recipe
                .steps
                .iter()
                .map(|step| step.call.to.as_str())
                .chain(recipe.pairs.iter().map(|pair| pair.to.as_str()))
        }) {
            if !own(to) && !reaches.iter().any(|held| held == to) {
                reaches.push(to.to_owned());
            }
        }
        Self {
            upstream: manifest.plugin.upstream.clone(),
            license: manifest.plugin.license.clone(),
            reviewed: false,
            claims: manifest
                .claims
                .iter()
                .map(|claim| claim.capability.clone())
                .collect(),
            overrides: super::stating::overrides(manifest),
            reaches,
            secrets: manifest
                .secrets
                .iter()
                .map(|secret| Secret {
                    id: secret.id.clone(),
                    of: secret.of.clone(),
                    why: secret.why.clone(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests;
