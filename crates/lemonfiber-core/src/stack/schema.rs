//! The published schemas of a stack's manifest files: the root, `stack.toml`, and one
//! service's file, `services/<id>.toml`.
//!
//! Generated from the types lemonfiber reads them into, for the reason the plugin
//! manifest's schema is: a hand-written one is a second description of the contract,
//! free to disagree with the parser. A claim is the plugin contract's shape, so its
//! definition here is the plugin manifest's own, put in place of the open table the
//! manifest crate is able to describe.

use schemars::Schema;
use serde_json::Value;

/// Where the generated schema of `stack.toml` is kept, relative to the workspace root.
pub const ROOT_PATH: &str = "contract/stack-manifest.schema.json";

/// Where the generated schema of a service's file is kept, relative to the workspace
/// root.
pub const SERVICE_PATH: &str = "contract/stack-service.schema.json";

/// The plugin manifest's definition of a claim, which the stack's claims take.
const CLAIM: &str = "Claim";

/// What a definition is referred to by, inside one schema document.
const DEFINED: &str = "#/$defs/";

/// The schema of `stack.toml`, as the artefact is committed.
#[must_use]
pub fn root() -> Option<String> {
    crate::plugin::rendered(&with_claims(lemonfiber_manifest::assembly::root_schema()))
}

/// The schema of one service's file, as the artefact is committed.
#[must_use]
pub fn service() -> Option<String> {
    crate::plugin::rendered(&with_claims(lemonfiber_manifest::assembly::service_schema()))
}

/// `schema` with the plugin manifest's definition of a claim, and every definition it
/// refers to, in place of the open table.
fn with_claims(mut schema: Schema) -> Schema {
    let plugin = schemars::schema_for!(lemonfiber_plugin::Manifest);
    let theirs = plugin
        .get("$defs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(defs) = schema.get_mut("$defs").and_then(Value::as_object_mut) {
        let mut wanted = vec![CLAIM.to_owned()];
        while let Some(name) = wanted.pop() {
            if let (false, Some(definition)) = (defs.contains_key(&name), theirs.get(&name)) {
                referred(definition, &mut wanted);
                defs.insert(name, definition.clone());
            }
        }
        defs.insert(
            lemonfiber_manifest::CLAIM.to_owned(),
            serde_json::json!({ "$ref": format!("{DEFINED}{CLAIM}") }),
        );
    }
    schema
}

/// Every definition `value` refers to, by name.
fn referred(value: &Value, into: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            if let Some(name) = fields
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|at| at.strip_prefix(DEFINED))
            {
                into.push(name.to_owned());
            }
            fields.values().for_each(|inner| referred(inner, into));
        }
        Value::Array(each) => each.iter().for_each(|inner| referred(inner, into)),
        _ => {}
    }
}

#[cfg(test)]
mod tests;
