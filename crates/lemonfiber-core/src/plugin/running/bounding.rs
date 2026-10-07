//! Where each value a run holds may be sent, held again at every call.
//!
//! **Reading the manifest is the first check, and this is the second.** Each value
//! carries the destinations its pairs name. A value the credential store holds for a
//! service is narrowed to that service, and so is every value a step captures from a
//! call that carried one: what a credential is traded for is held as the credential is.
//! A value bound for a host outside the stack goes only where this act approved it, as
//! `<value>@<destination>`. A call carrying a value anywhere else is not sent.
//!
//! **What a step traded is read off what it sent.** A capture is held to a service
//! because the call that took it carried a value held to that service, as the call was
//! actually made, so a step a guard skipped trades nothing.

use std::collections::{BTreeMap, BTreeSet};

use lemonfiber_plugin::{Origin, Recipe};

/// Where every value of one run may be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Bounds<'a> {
    /// The destinations each value's pairs name, by its name.
    pairs: BTreeMap<&'a str, BTreeSet<&'a str>>,
    /// The service each value is held to, where it is held to one.
    held: BTreeMap<String, String>,
    /// Every pair to a host outside the stack this act approved.
    approved: &'a [String],
}

impl<'a> Bounds<'a> {
    /// The bounds a run of this recipe starts with, under these approvals.
    pub(super) fn of(recipe: &'a Recipe, approved: &'a [String]) -> Self {
        let mut pairs: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for pair in &recipe.pairs {
            pairs.entry(&pair.value).or_default().insert(&pair.to);
        }
        let held = recipe
            .inputs
            .iter()
            .filter(|input| input.origin == Origin::CredentialStore)
            .filter_map(|input| input.of.clone().map(|of| (input.name.clone(), of)))
            .collect();
        Self {
            pairs,
            held,
            approved,
        }
    }

    /// Why this value may not be sent to `to`, or nothing where it may.
    pub(super) fn withheld(&self, name: &str, to: &str, outside: bool) -> Option<String> {
        if let Some(owner) = self.held.get(name).filter(|owner| *owner != to) {
            return Some(format!(
                "carries {name} to {to}, and {name} is held to {owner}, whose credential it is \
                 or was traded for"
            ));
        }
        if !self.pairs.get(name).is_some_and(|tos| tos.contains(to)) {
            return Some(format!(
                "carries {name} to {to}, and no pair of the recipe declares that"
            ));
        }
        let approval = crate::plugin::recipes::approval(name, to);
        (outside && !self.approved.contains(&approval)).then(|| {
            format!(
                "carries {name} to {to}, outside the stack, and {approval} was not approved \
                 for this act"
            )
        })
    }

    /// Hold every value a step captured to the service a value its call carried is held
    /// to, where one is.
    pub(super) fn traded<'b>(
        &mut self,
        carried: &BTreeSet<String>,
        captured: impl Iterator<Item = &'b String>,
    ) {
        let owner = carried.iter().find_map(|name| self.held.get(name)).cloned();
        if let Some(owner) = owner {
            self.held
                .extend(captured.map(|name| (name.clone(), owner.clone())));
        }
    }
}

#[cfg(test)]
mod tests;
