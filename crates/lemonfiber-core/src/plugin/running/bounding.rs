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
    /// The service each value is held to by how it came, where it is held to one.
    held: BTreeMap<String, String>,
    /// Every pair to a host outside the stack this act approved.
    approved: &'a [String],
    /// Every credential lemonfiber holds, by the service it is for, which a value
    /// holding one is held to whatever its name.
    credentials: &'a [(String, String)],
}

impl<'a> Bounds<'a> {
    /// The bounds a run of this recipe starts with, under these approvals and with these
    /// credentials held.
    pub(super) fn of(
        recipe: &'a Recipe,
        approved: &'a [String],
        credentials: &'a [(String, String)],
    ) -> Self {
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
            credentials,
        }
    }

    /// The services this value is held to: by how it came, and by every credential it
    /// holds, whole or inside a longer value.
    fn owners<'b>(&'b self, name: &str, value: &str) -> impl Iterator<Item = (&'b str, bool)> {
        let came = self.held.get(name).map(|owner| (owner.as_str(), false));
        let holds = self
            .credentials
            .iter()
            .filter(|(_, secret)| !secret.is_empty() && value.contains(secret.as_str()))
            .map(|(owner, _)| (owner.as_str(), true));
        came.into_iter().chain(holds.collect::<Vec<_>>())
    }

    /// Why a value may not go toward `to`, where it is held to another service, said
    /// after what the step does with it; never what it holds.
    fn elsewhere(&self, does: &str, name: &str, value: &str, to: &str) -> Option<String> {
        let (owner, holding) = self.owners(name, value).find(|(owner, _)| *owner != to)?;
        Some(if holding {
            format!("{does}, and what {name} holds is the credential lemonfiber holds for {owner}")
        } else {
            format!(
                "{does}, and {name} is held to {owner}, whose credential it is or was traded for"
            )
        })
    }

    /// Why this value may not be sent to `to`, or nothing where it may.
    pub(super) fn withheld(
        &self,
        name: &str,
        value: &str,
        to: &str,
        outside: bool,
    ) -> Option<String> {
        if let Some(why) = self.elsewhere(&format!("carries {name} to {to}"), name, value, to) {
            return Some(why);
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

    /// Why a step whose guard reads this value may not be made toward `to`, or nothing
    /// where it may: a guard decides on what it reads as surely as a call carries it.
    pub(super) fn decided(&self, name: &str, value: &str, to: &str) -> Option<String> {
        self.elsewhere(
            &format!("decides on {name} for a call to {to}"),
            name,
            value,
            to,
        )
    }

    /// Hold every value a step captured to the service a value its call carried, or its
    /// guard read, is held to, where one is.
    pub(super) fn traded<'b>(
        &mut self,
        carried: &BTreeMap<String, String>,
        captured: impl Iterator<Item = &'b String>,
    ) {
        let owner = carried
            .iter()
            .find_map(|(name, value)| self.owners(name, value).next())
            .map(|(owner, _)| owner.to_owned());
        if let Some(owner) = owner {
            self.held
                .extend(captured.map(|name| (name.clone(), owner.clone())));
        }
    }
}

#[cfg(test)]
mod tests;
