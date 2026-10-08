//! Every action this surface takes, as the contract lists it.
//!
//! A client generating a method per action needs what a request body may carry for
//! each, and what each asks before it writes. Both are read off what the surface
//! already refuses by: the arguments from the table every request is held to, their
//! types from the carrier that deserialises them, and the consent from which of those
//! arguments carry a yes. So an action or an argument the surface gains is one every
//! client generates, and none is written down a second time here.

use serde::Serialize;
use serde_json::Value;

use super::asked::{Arguments, TAKEN};
use super::named::OFFERED;

/// The arguments that carry an operator's yes: an agreement given in advance, the
/// offer a yes was read in and what of it was agreed to, and each value a plugin's
/// recipe may carry elsewhere that was approved.
pub const CONSENT: &[&str] = &["confirm", "offer", "agreed", "approved"];

/// One action, as the contract lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Action {
    /// The action, as `POST /api/actions/<action>` names it.
    pub action: &'static str,
    /// Every argument it takes, in the order the carrier declares them. Empty for an
    /// action that takes nothing, which refuses any argument at all.
    pub arguments: Vec<Argument>,
    /// The arguments among those that carry the operator's yes, which a client asks
    /// for before it sends the action. Empty for an action that asks nothing first.
    pub consent: Vec<&'static str>,
    /// Whether it takes `dry_run`, so a client can rehearse it and offer the real call
    /// after.
    pub rehearsal: bool,
}

/// One argument an action takes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Argument {
    /// Its name, as a request body spells it.
    pub name: &'static str,
    /// Its type, as the carrier that reads it declares it.
    #[serde(rename = "type")]
    pub shape: Value,
}

/// Every action this surface offers, in the order it offers them, each with the
/// arguments it takes and the consent it asks for; `rehearsable` says which take a
/// rehearsal, as the core decides for the command each reaches.
#[must_use]
pub fn every(rehearsable: impl Fn(&str) -> bool) -> Vec<Action> {
    let carrier = serde_json::to_value(schemars::schema_for!(Arguments)).ok();
    let typed = |name: &str| {
        carrier
            .as_ref()
            .and_then(|schema| schema.get("properties"))
            .and_then(|properties| properties.get(name))
            .cloned()
            .unwrap_or(Value::Null)
    };
    OFFERED
        .iter()
        .map(|action| {
            let arguments: Vec<Argument> = TAKEN
                .iter()
                .filter(|taken| taken.takers.contains(action))
                .map(|taken| Argument {
                    name: taken.name,
                    shape: typed(taken.name),
                })
                .collect();
            Action {
                action,
                consent: arguments
                    .iter()
                    .map(|argument| argument.name)
                    .filter(|name| CONSENT.contains(name))
                    .collect(),
                rehearsal: rehearsable(action),
                arguments,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
