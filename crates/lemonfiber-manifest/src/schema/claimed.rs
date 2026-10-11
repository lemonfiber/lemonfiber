//! A claim, kept as the table the stack wrote.

use serde::Deserialize;

/// One `[[service.claim]]` table, exactly as the stack wrote it.
///
/// Compared as TOML values compare. `Eq` is claimed although a TOML float is not
/// reflexive for `NaN`: a claim carries statuses, paths and names, and a `NaN` in one
/// is a claim the plugin contract's reader refuses before anything compares it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct Claimed(pub toml::Table);

impl Eq for Claimed {}

/// Described as an open table here, because a claim is the plugin contract's shape and
/// this crate cannot reach that contract's types. What publishes the stack's schema
/// sees both, and puts the plugin manifest's own definition of a claim in its place.
impl schemars::JsonSchema for Claimed {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        CLAIM.into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
            "description": "A claim, written as the plugin manifest writes one."
        })
    }
}

/// The name a claim's definition is published under in the stack's schema.
pub const CLAIM: &str = "StackClaim";
