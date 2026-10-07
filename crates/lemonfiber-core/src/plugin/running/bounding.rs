//! Where each value a run holds may be sent, held again at every call.
//!
//! **Reading the manifest is the first check, and this is the second.** Each value
//! carries the destinations its pairs name. A value the credential store holds for a
//! service is narrowed to that service, and so is any value holding one, whatever its
//! name. A value a step captures from a call carrying one is held as the credential
//! is. A value a step captures from the answer of a service in this stack otherwise is
//! held to that service, and goes elsewhere only where a pair releases it and this act
//! approved that pair. A value bound for a host outside the stack goes only where this act
//! approved it, as `<value>@<destination>`. A call carrying a value anywhere else is not
//! sent.
//!
//! **What a step captured is held by what it sent and who answered,** as the call was
//! actually made, so a step a guard skipped captures and holds nothing. A capture from a
//! call carrying a credential is held as the credential is, and no release frees it.

use std::collections::{BTreeMap, BTreeSet};

use lemonfiber_plugin::{Origin, Recipe};

/// How a value came to be held to one service, weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Hold {
    /// It was captured from that service's answer by a call that carried no credential.
    Answered,
    /// It was captured from a call that carried a credential held to it, and is held as
    /// the credential is: no release frees what a credential bought.
    Traded,
    /// It is the credential lemonfiber holds for it, by its input or by what it holds.
    Credential,
}

/// Where every value of one run may be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Bounds<'a> {
    /// The destinations each value's pairs name, by its name.
    pairs: BTreeMap<&'a str, BTreeSet<&'a str>>,
    /// Every pair carrying a release, as its value and its destination.
    released: BTreeSet<(&'a str, &'a str)>,
    /// The service each value is held to by how it came, where it is held to one.
    held: BTreeMap<String, (String, Hold)>,
    /// Every pair to a host outside the stack, or carrying a release, this act approved.
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
        let released = recipe
            .pairs
            .iter()
            .filter(|pair| pair.release.is_some())
            .map(|pair| (pair.value.as_str(), pair.to.as_str()))
            .collect();
        let held = recipe
            .inputs
            .iter()
            .filter(|input| input.origin == Origin::CredentialStore)
            .filter_map(|input| {
                input
                    .of
                    .clone()
                    .map(|of| (input.name.clone(), (of, Hold::Credential)))
            })
            .collect();
        Self {
            pairs,
            released,
            held,
            approved,
            credentials,
        }
    }

    /// The services this value is held to: by how it came, and by every credential it
    /// holds, whole or inside a longer value.
    fn owners<'b>(&'b self, name: &str, value: &str) -> impl Iterator<Item = (&'b str, Hold)> {
        let came = self
            .held
            .get(name)
            .map(|(owner, hold)| (owner.as_str(), *hold));
        let holds = self
            .credentials
            .iter()
            .filter(|(_, secret)| !secret.is_empty() && value.contains(secret.as_str()))
            .map(|(owner, _)| (owner.as_str(), Hold::Credential));
        came.into_iter().chain(holds.collect::<Vec<_>>())
    }

    /// Why a value may not go toward `to`, where it is held to another service, said
    /// after what the step does with it; never what it holds.
    fn elsewhere(&self, does: &str, name: &str, value: &str, to: &str) -> Option<String> {
        self.owners(name, value)
            .filter(|(owner, _)| *owner != to)
            .find_map(|(owner, hold)| match hold {
                Hold::Credential
                    if self
                        .held
                        .get(name)
                        .is_some_and(|(_, how)| *how == Hold::Credential) =>
                {
                    Some(format!(
                        "{does}, and {name} is the credential lemonfiber holds for {owner}"
                    ))
                }
                Hold::Credential => Some(format!(
                    "{does}, and what {name} holds is the credential lemonfiber holds for {owner}"
                )),
                Hold::Traded => Some(format!(
                    "{does}, and {name} was traded for the credential lemonfiber holds for {owner}"
                )),
                Hold::Answered => self.unreleased(does, name, owner, to),
            })
    }

    /// Why a capture from `owner`'s answer may not go to `to`: no pair releases it there,
    /// or this act did not approve the one that does.
    fn unreleased(&self, does: &str, name: &str, owner: &str, to: &str) -> Option<String> {
        if !self.released.contains(&(name, to)) {
            return Some(format!(
                "{does}, and {name} was captured from the answer of {owner}, and no pair \
                 releases it to {to}"
            ));
        }
        let approval = crate::plugin::recipes::approval(name, to);
        (!self.approved.contains(&approval)).then(|| {
            format!(
                "{does}, and {name} was captured from the answer of {owner}, and releasing it \
                 as {approval} was not approved for this act"
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

    /// Hold every value a step captured: as the credential is, where its call carried or
    /// its guard read one, or else to the service in this stack that answered, where
    /// `by` is one.
    pub(super) fn captured<'b>(
        &mut self,
        carried: &BTreeMap<String, String>,
        by: Option<&str>,
        captured: impl Iterator<Item = &'b String>,
    ) {
        let traded = carried.iter().find_map(|(name, value)| {
            self.owners(name, value)
                .find(|(_, hold)| *hold != Hold::Answered)
                .map(|(owner, _)| owner.to_owned())
        });
        let holding = match (traded, by) {
            (Some(owner), _) => (owner, Hold::Traded),
            (None, Some(by)) => (by.to_owned(), Hold::Answered),
            (None, None) => return,
        };
        for name in captured {
            // A hold only ever grows stronger: a value captured again is never freed by it.
            if self
                .held
                .get(name)
                .is_none_or(|(_, hold)| *hold < holding.1)
            {
                self.held.insert(name.clone(), holding.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests;
