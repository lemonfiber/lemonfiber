//! The variables an earlier release kept a setting's or a secret's value under, as each
//! names its own in `was`.

use std::collections::BTreeMap;

use super::values::{Secret, Value};
use super::words::is_variable;
use super::NAMESPACED;

/// One variable an earlier release kept a value under, and what names it in `was`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Earlier {
    /// The variable's name in the stack's environment file.
    pub key: String,
    /// The setting, or the secret as `<namespace>:<id>`, that names it.
    pub name: String,
    /// Whether a secret names it.
    pub secret: bool,
}

impl Earlier {
    /// What names it, as a refusal says so.
    #[must_use]
    pub fn named(&self) -> String {
        let kind = if self.secret { "secret" } else { "setting" };
        format!("{kind} {}", self.name)
    }
}

/// Every variable a setting `env` reads names in `was`, then every one a secret of
/// `secrets` names, each secret named within `namespace`.
#[must_use]
pub fn of<'a>(
    env: impl IntoIterator<Item = &'a BTreeMap<String, Value>>,
    namespace: &str,
    secrets: &[Secret],
) -> Vec<Earlier> {
    distinct(
        settings(env).into_iter().chain(
            secrets
                .iter()
                .filter_map(|held| secret(namespace, &held.id, held.was.as_deref())),
        ),
    )
}

/// Every variable a setting `env` reads names in `was`.
#[must_use]
pub fn settings<'a>(env: impl IntoIterator<Item = &'a BTreeMap<String, Value>>) -> Vec<Earlier> {
    env.into_iter()
        .flat_map(BTreeMap::values)
        .filter_map(|value| match value {
            Value::Setting(setting) => setting.was.as_ref().map(|key| Earlier {
                key: key.clone(),
                name: setting.setting.clone(),
                secret: false,
            }),
            _ => None,
        })
        .collect()
}

/// The variable the secret `id` within `namespace` names in `was`, where it names one.
#[must_use]
pub fn secret(namespace: &str, id: &str, was: Option<&str>) -> Option<Earlier> {
    was.map(|key| Earlier {
        key: key.to_owned(),
        name: format!("{namespace}{NAMESPACED}{id}"),
        secret: true,
    })
}

/// `earlier` with each repeat after the first left out.
#[must_use]
pub fn distinct(earlier: impl IntoIterator<Item = Earlier>) -> Vec<Earlier> {
    let mut found: Vec<Earlier> = Vec::new();
    for one in earlier {
        if !found.contains(&one) {
            found.push(one);
        }
    }
    found
}

/// Each pair of settings or secrets naming one variable, the earlier of `earlier` first.
#[must_use]
pub fn named_twice(earlier: &[Earlier]) -> Vec<(&Earlier, &Earlier)> {
    earlier
        .iter()
        .enumerate()
        .flat_map(|(at, one)| {
            earlier
                .iter()
                .skip(at + 1)
                .filter(move |other| other.key == one.key && other.name != one.name)
                .map(move |other| (one, other))
        })
        .collect()
}

/// Each of `own` naming a variable one of `others` names for another setting or secret,
/// with that other.
#[must_use]
pub fn clashing<'a>(own: &'a [Earlier], others: &'a [Earlier]) -> Vec<(&'a Earlier, &'a Earlier)> {
    own.iter()
        .flat_map(|one| {
            others
                .iter()
                .filter(move |other| other.key == one.key && other.name != one.name)
                .map(move |other| (one, other))
        })
        .collect()
}

/// Every rule what `earlier` names breaks, each as where and what: a name that is not a
/// variable's, and one variable named by two.
#[must_use]
pub fn broken(earlier: &[Earlier]) -> Vec<(String, String)> {
    let unnamed = earlier
        .iter()
        .filter(|one| !is_variable(&one.key))
        .map(|one| {
            (
                format!("{}.was", one.named()),
                format!(
                "{} is not upper-case letters, digits and \"_\", starting with a letter or \"_\"",
                one.key
            ),
            )
        });
    let twice = named_twice(earlier).into_iter().map(|(one, other)| {
        (
            format!("{}.was", other.named()),
            format!(
                "{} is named by {} and {}",
                one.key,
                one.named(),
                other.named()
            ),
        )
    });
    unnamed.chain(twice).collect()
}

#[cfg(test)]
mod tests;
