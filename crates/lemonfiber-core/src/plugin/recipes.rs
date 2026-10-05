//! What a plugin's recipes would do, as an operator agrees to it: the calls in order,
//! the adapter each reaches through, and every value that would leave for somewhere
//! else.
//!
//! Read off the manifest at install and kept on the record, for the reason everything
//! else there is: the author's directory may be gone tomorrow, and what a plugin may
//! send where is a question about this machine rather than about a document.
//!
//! **Every list is present, empty where there is nothing in it.** A record that left a
//! list out would read the same as one that had nothing in it, and *none* and *not
//! read* are two different things to tell somebody.
//!
//! **An adapter is always lemonfiber's.** A plugin names one from the closed set this
//! build implements and can bring none of its own, so every adapter here says so in a
//! field of its own rather than leaving a client to know it.

use lemonfiber_manifest::ApiKind;
use lemonfiber_plugin::Manifest;
use serde::{Deserialize, Serialize};

/// Whose an adapter is.
///
/// One answer, and the field exists so that the answer is on the wire: an adapter a
/// plugin could bring would be code a stranger wrote running with lemonfiber's
/// authority, which nothing here can load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "PluginAdapterOwner")]
pub enum Owner {
    /// lemonfiber's own, from the set `contract/adapters.json` publishes.
    Lemonfiber,
}

/// The adapter a call reaches its destination through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginStepAdapter")]
pub struct Adapter {
    /// Which of lemonfiber's adapters it is.
    pub kind: ApiKind,
    /// Whose it is, which is lemonfiber's.
    pub owner: Owner,
}

/// One call a recipe makes, in the order it makes them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginStep")]
pub struct Step {
    /// The step's id within its recipe.
    pub id: String,
    /// The HTTP method it calls with.
    pub method: String,
    /// Where it calls: a service of this stack's or the plugin's own, or a name outside
    /// both, as the manifest wrote it. Never a resolved address.
    pub to: String,
    /// The path it calls.
    pub path: String,
    /// The adapter the destination is reached through, or nothing where it is a name
    /// outside the stack, which no adapter of lemonfiber's speaks to.
    pub adapter: Option<Adapter>,
}

/// One value a recipe could carry to one destination, as the operator agrees to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginPair")]
pub struct Pair {
    /// What the value is called within the recipe.
    pub value: String,
    /// Whose value it is, as the step that captures it says; empty where no step does.
    pub origin: String,
    /// Where it may be carried, by the name the manifest gives it.
    pub to: String,
    /// What approving this pair is written as, on the command line and over the web.
    pub approval: String,
}

/// One recipe, as an operator agrees to what it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginRecipe")]
pub struct Recipe {
    /// The recipe's id within the plugin.
    pub id: String,
    /// What it accomplishes, in one line.
    pub title: String,
    /// Why it is worth running.
    pub why: String,
    /// Every call, in the order the recipe makes them.
    pub steps: Vec<Step>,
    /// Every value it could carry, and where to.
    pub pairs: Vec<Pair>,
}

/// One adapter of lemonfiber's that one of the plugin's own services names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginServiceAdapter")]
pub struct Named {
    /// The service that names it.
    pub service: String,
    /// Which of lemonfiber's adapters it is.
    pub kind: ApiKind,
    /// Whose it is, which is lemonfiber's.
    pub owner: Owner,
}

/// Every adapter of lemonfiber's the plugin's own services name, in the order the
/// manifest declares the services.
#[must_use]
pub fn named(manifest: &Manifest) -> Vec<Named> {
    manifest
        .services
        .iter()
        .filter_map(|service| {
            service.api.as_ref().map(|api| Named {
                service: service.id.clone(),
                kind: api.kind,
                owner: Owner::Lemonfiber,
            })
        })
        .collect()
}

/// What approving one value's carriage to one destination is written as.
///
/// The value first and the destination after an `@`, because a destination is a
/// service id or a host name and neither holds one: the two halves are always where
/// the one `@` puts them.
#[must_use]
pub fn approval(value: &str, to: &str) -> String {
    format!("{value}@{to}")
}

/// Every recipe this manifest declares, with the adapter of each call to one of the
/// plugin's own services.
///
/// A call to a service of the stack's is given its adapter by [`reaching`], which has
/// the stack to read it from; read here, it would be a call outside the plugin with no
/// way to tell a bundled service from a host on the internet.
#[must_use]
pub fn declared(manifest: &Manifest) -> Vec<Recipe> {
    let own = |to: &str| {
        manifest
            .services
            .iter()
            .find(|service| service.id == to)
            .and_then(|service| service.api.as_ref())
            .map(|api| api.kind)
    };
    manifest
        .recipes
        .iter()
        .map(|recipe| Recipe {
            id: recipe.id.clone(),
            title: recipe.title.clone(),
            why: recipe.why.clone(),
            steps: recipe
                .steps
                .iter()
                .map(|step| Step {
                    id: step.id.clone(),
                    method: step.call.method.clone(),
                    to: step.call.to.clone(),
                    path: step.call.path.clone(),
                    adapter: own(&step.call.to).map(lemonfibers),
                })
                .collect(),
            pairs: recipe
                .pairs
                .iter()
                .map(|pair| Pair {
                    value: pair.value.clone(),
                    origin: recipe
                        .steps
                        .iter()
                        .flat_map(|step| step.capture.iter())
                        .find(|capture| capture.name == pair.value)
                        .map(|capture| capture.origin.clone())
                        .unwrap_or_default(),
                    to: pair.to.clone(),
                    approval: approval(&pair.value, &pair.to),
                })
                .collect(),
        })
        .collect()
}

/// The same recipes, with the adapter of every call to one of the stack's services.
#[must_use]
pub fn reaching(mut recipes: Vec<Recipe>, stack: &lemonfiber_manifest::Manifest) -> Vec<Recipe> {
    for step in recipes
        .iter_mut()
        .flat_map(|recipe| recipe.steps.iter_mut())
    {
        if step.adapter.is_none() {
            step.adapter = stack
                .services
                .iter()
                .find(|service| service.id == step.to)
                .and_then(|service| service.api.as_ref())
                .map(|api| lemonfibers(api.kind));
        }
    }
    recipes
}

/// Every approval these recipes ask for, once each, in the order they declare them.
#[must_use]
pub fn approvals(recipes: &[Recipe]) -> Vec<&str> {
    let mut asked: Vec<&str> = Vec::new();
    for pair in recipes.iter().flat_map(|recipe| recipe.pairs.iter()) {
        if !asked.contains(&pair.approval.as_str()) {
            asked.push(&pair.approval);
        }
    }
    asked
}

/// One of lemonfiber's adapters, said to be lemonfiber's.
const fn lemonfibers(kind: ApiKind) -> Adapter {
    Adapter {
        kind,
        owner: Owner::Lemonfiber,
    }
}

#[cfg(test)]
mod tests;
