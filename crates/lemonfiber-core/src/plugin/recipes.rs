//! What a plugin's recipes would do, as an operator agrees to it: the calls in order,
//! the API each reaches its service in, and every value that would leave for somewhere
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
//! **A step's API is its service's declaration.** A call to a service of the stack's
//! reaches it in the API the installed plugin providing that service declares as its
//! `native`, read as declared rather than discovered, and in none where it declares none.

use lemonfiber_plugin::{Manifest, On, Origin};
use serde::{Deserialize, Serialize};

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
    /// The API the destination is reached in, as the installed plugin providing it
    /// declares it, or nothing where none does or it is a name outside the stack.
    pub native: Option<String>,
}

/// One value a recipe could carry to one destination, as the operator agrees to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginPair")]
pub struct Pair {
    /// What the value is called within the recipe.
    pub value: String,
    /// Whose value it is, as its input or the step that captures it says; empty where
    /// neither does.
    pub origin: String,
    /// Where it may be carried, by the name the manifest gives it.
    pub to: String,
    /// What approving this pair is written as, on the command line and over the web,
    /// where it carries the value to a host outside the stack or carries a release.
    /// Absent on any other pair to a service in this stack, which takes nothing off the
    /// machine or away from the service it came from, and asks for no approval.
    // Described as a string that may be absent rather than as a nullable one: it is
    // left out, never sent as null. Not a doc comment, which schemars would publish.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub approval: Option<String>,
    /// Why the value is carried away from the service it was read from, in the manifest's
    /// own sentence, where the pair releases it. Absent on every other pair.
    // Left out rather than sent as null, as `approval` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub release: Option<String>,
    /// The service in this stack the released value was read from, where the pair
    /// releases it. Absent on every other pair.
    // Left out rather than sent as null, as `approval` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub from: Option<String>,
}

/// One value a recipe asks the operator for when it runs.
///
/// Never the value: a reading says what will be asked, and what is given for it is
/// handed to the recipe and said back by nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginRecipeInput")]
pub struct RecipeInput {
    /// What the input is called, which is what a value is given for it under.
    pub name: String,
    /// What the operator is asked for it.
    pub ask: String,
    /// Whether the value may be a secret, so a surface takes it without showing it.
    pub secret: bool,
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
    /// When it runs: as part of an install and an update, or only when asked for by name.
    #[serde(default)]
    pub on: On,
    /// Every value it asks the operator for when it runs, in the order it declares them.
    #[serde(default)]
    pub inputs: Vec<RecipeInput>,
    /// Every call, in the order the recipe makes them.
    pub steps: Vec<Step>,
    /// Every value it could carry, and where to.
    pub pairs: Vec<Pair>,
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

/// Every recipe this manifest declares, each call yet to be given its API.
///
/// A call to a service of the stack's is given its API by [`reaching`], which has
/// the stack to read it from; read here, it would be a call outside the plugin with no
/// way to tell a bundled service from a host on the internet.
#[must_use]
pub fn declared(manifest: &Manifest) -> Vec<Recipe> {
    manifest
        .recipes
        .iter()
        .map(|recipe| Recipe {
            id: recipe.id.clone(),
            title: recipe.title.clone(),
            why: recipe.why.clone(),
            on: recipe.on,
            inputs: recipe
                .inputs
                .iter()
                .filter(|input| input.origin == Origin::Operator)
                .map(|input| RecipeInput {
                    name: input.name.clone(),
                    ask: input.question().to_owned(),
                    secret: input.secret,
                })
                .collect(),
            steps: recipe
                .steps
                .iter()
                .map(|step| Step {
                    id: step.id.clone(),
                    method: step.call.method.clone(),
                    to: step.call.to.clone(),
                    path: step.call.path.clone(),
                    native: None,
                })
                .collect(),
            pairs: recipe
                .pairs
                .iter()
                .map(|pair| Pair {
                    value: pair.value.clone(),
                    origin: origin_of(recipe, &pair.value)
                        .map(|origin| origin.written().to_owned())
                        .unwrap_or_default(),
                    to: pair.to.clone(),
                    approval: (pair.release.is_some()
                        || lemonfiber_plugin::outside(manifest, &pair.to))
                    .then(|| approval(&pair.value, &pair.to)),
                    release: pair.release.clone(),
                    from: pair
                        .release
                        .as_ref()
                        .and_then(|_| read_from(recipe, &pair.value))
                        .map(str::to_owned),
                })
                .collect(),
        })
        .collect()
}

/// Where a value one recipe has comes from: the input that brings it in, or the step
/// that captures it.
fn origin_of(recipe: &lemonfiber_plugin::Recipe, value: &str) -> Option<lemonfiber_plugin::Origin> {
    recipe
        .inputs
        .iter()
        .find(|input| input.name == value)
        .map(|input| input.origin)
        .or_else(|| {
            recipe
                .steps
                .iter()
                .flat_map(|step| step.capture.iter())
                .find(|capture| capture.name == value)
                .map(|capture| capture.origin)
        })
}

/// The service whose answer a step of this recipe captures `value` from.
fn read_from<'a>(recipe: &'a lemonfiber_plugin::Recipe, value: &str) -> Option<&'a str> {
    recipe
        .steps
        .iter()
        .find(|step| step.capture.iter().any(|capture| capture.name == value))
        .map(|step| step.call.to.as_str())
}

/// The same recipes, with the API of every call to one of the stack's services: the one
/// the plugin of `installed` providing that service declares as its `native`.
#[must_use]
pub fn reaching(mut recipes: Vec<Recipe>, installed: &[super::Installed]) -> Vec<Recipe> {
    for step in recipes
        .iter_mut()
        .flat_map(|recipe| recipe.steps.iter_mut())
    {
        if step.native.is_none() {
            step.native = declared_native(installed, &step.to);
        }
    }
    recipes
}

/// The API the service `service` of one of `installed` declares as its `native`, where
/// one provides it and declares one.
fn declared_native(installed: &[super::Installed], service: &str) -> Option<String> {
    installed
        .iter()
        .flat_map(|one| &one.services)
        .find(|placed| placed.service == service)?
        .native
        .clone()
}

/// Every approval these recipes ask for, once each, in the order they declare them.
#[must_use]
pub fn approvals(recipes: &[Recipe]) -> Vec<&str> {
    let mut asked: Vec<&str> = Vec::new();
    for approval in recipes
        .iter()
        .flat_map(|recipe| recipe.pairs.iter())
        .filter_map(|pair| pair.approval.as_deref())
    {
        if !asked.contains(&approval) {
            asked.push(approval);
        }
    }
    asked
}

#[cfg(test)]
mod tests;
