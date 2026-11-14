//! The plugins whose own profiles join a plan, and those naming forms the stack no longer
//! declares.

use lemonfiber_manifest::Manifest;

use super::Plan;

/// A plugin naming forms the stack does not declare.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Vanished {
    /// The plugin, by its id.
    pub plugin: String,
    /// The forms it names that the stack does not declare, in the order it names them.
    pub forms: Vec<String>,
}

/// A plugin whose services sit in a profile of their own, and the forms that profile
/// joins the closure of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joining {
    /// The plugin, by its id.
    pub plugin: String,
    /// The profile its services sit in.
    pub profile: String,
    /// Its services, in the order it declares them.
    pub services: Vec<String>,
    /// The forms its manifest names; none where its record kept none, which joins every
    /// form.
    pub forms: Vec<String>,
}

impl Joining {
    /// Whether it runs with `forms` of `manifest`'s, where naming none runs every form.
    #[must_use]
    pub fn runs_with(&self, forms: &[String], manifest: &Manifest) -> bool {
        if self.forms.is_empty() {
            return true;
        }
        if forms.is_empty() {
            return self
                .forms
                .iter()
                .any(|form| manifest.forms.iter().any(|one| &one.id == form));
        }
        self.forms.iter().any(|form| forms.contains(form))
    }
}

/// `plan` with each of `plugins` added where the plan runs a form it names: its profile to
/// the profiles and its services after the stack's. Where the plan names no form, every
/// form runs. Each plugin naming a form `manifest` does not declare is named in
/// [`Plan::vanished`].
#[must_use]
pub fn joined(mut plan: Plan, manifest: &Manifest, plugins: &[Joining]) -> Plan {
    let declared = |form: &str| manifest.forms.iter().any(|one| one.id == form);
    for plugin in plugins {
        let gone: Vec<String> = plugin
            .forms
            .iter()
            .filter(|form| !declared(form))
            .cloned()
            .collect();
        if !gone.is_empty() {
            plan.vanished.push(Vanished {
                plugin: plugin.plugin.clone(),
                forms: gone,
            });
        }
        if plugin.runs_with(&plan.forms, manifest) && plan.profiles.insert(plugin.profile.clone()) {
            plan.services.extend(plugin.services.iter().cloned());
        }
    }
    plan
}
