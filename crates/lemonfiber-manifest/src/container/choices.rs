//! The values a setting's service accepts, and the values of other settings a reference
//! or a secret is read under.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::values::{Constant, Secret, Value};

/// The values of other settings under which a reference is written, or a secret asked for
/// and held: each setting by name, with the values it has to hold one of.
pub type Needs = BTreeMap<String, Vec<Constant>>;

/// One value a setting accepts.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(rename = "SettingChoice")]
pub enum Choice {
    /// A value that is always a choice.
    Always(Constant),
    /// Values that are choices only where `needs` is met.
    Needing(Needing),
}

/// Values that are choices only where the other settings `needs` names hold one of the
/// values it lists.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "SettingChoiceNeeding")]
pub struct Needing {
    /// The values.
    pub values: Vec<Constant>,
    /// What they are choices under.
    pub needs: Needs,
}

impl Choice {
    /// Every value it lists.
    #[must_use]
    pub fn values(&self) -> &[Constant] {
        match self {
            Self::Always(value) => std::slice::from_ref(value),
            Self::Needing(needing) => &needing.values,
        }
    }

    /// What its values are choices under: nothing for a value that always is.
    #[must_use]
    pub fn needs(&self) -> Option<&Needs> {
        match self {
            Self::Always(_) => None,
            Self::Needing(needing) => Some(&needing.needs),
        }
    }
}

/// Whether `needs` is met: every setting it names holds one of the values it lists, as
/// `held` says what each holds.
#[must_use]
pub fn met(needs: &Needs, held: impl Fn(&str) -> Option<String>) -> bool {
    needs.iter().all(|(setting, values)| {
        held(setting).is_some_and(|value| values.iter().any(|one| one.text() == value))
    })
}

/// What `setting` holds in a service whose environment is `env` while the operator has
/// recorded no value: the default of its first reference to it, where it reads it.
#[must_use]
pub fn defaulted(env: &BTreeMap<String, Value>, setting: &str) -> Option<String> {
    env.values().find_map(|value| match value {
        Value::Setting(read) if read.setting == setting => Some(read.default.text()),
        _ => None,
    })
}

/// One service's settings and secrets, as the rules for choices read them.
#[derive(Debug, Clone, Copy)]
pub struct Reading<'a> {
    /// The service's id.
    pub service: &'a str,
    /// Its environment.
    pub env: &'a BTreeMap<String, Value>,
    /// The namespace its secrets are named in.
    pub namespace: &'a str,
    /// Every secret it holds.
    pub secrets: &'a [&'a Secret],
}

/// One setting's choices as a manifest declares them, and where.
struct Declared<'a> {
    /// Where the reference declaring them is.
    at: String,
    /// The choices.
    one_of: &'a [Choice],
}

/// Every rule what `services` declare about choices and needs breaks, each as where and
/// what: choices declared empty, with a value twice or on two references to one setting;
/// a default outside its setting's values; and needs naming a setting the service does
/// not read, the setting needing it, or a value outside that setting's choices.
#[must_use]
pub fn broken(services: &[Reading<'_>]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut declared: BTreeMap<&str, Declared<'_>> = BTreeMap::new();
    for (at, setting) in references(services) {
        let Some(one_of) = setting.one_of.as_deref() else {
            continue;
        };
        found.extend(listed(&at, &setting.setting, one_of));
        match declared.get(setting.setting.as_str()) {
            Some(first) => found.push((
                format!("{at}.one_of"),
                format!(
                    "declares the choices of setting {}, which {} declares already",
                    setting.setting, first.at
                ),
            )),
            None => {
                declared.insert(&setting.setting, Declared { at, one_of });
            }
        }
    }
    let choices = |setting: &str| -> Option<Vec<String>> {
        declared.get(setting).map(|first| {
            first
                .one_of
                .iter()
                .flat_map(Choice::values)
                .map(Constant::text)
                .collect()
        })
    };
    for service in services {
        for (name, value) in service.env {
            let Value::Setting(setting) = value else {
                continue;
            };
            let at = format!("service {}.env.{name}", service.service);
            if let (Some(values), None) = (choices(&setting.setting), &setting.recorded_with) {
                if !values.contains(&setting.default.text()) {
                    found.push((
                        format!("{at}.default"),
                        format!(
                            "{} is not one of setting {}'s values: {}",
                            setting.default.text(),
                            setting.setting,
                            values.join(", ")
                        ),
                    ));
                }
            }
            let inner = setting
                .one_of
                .iter()
                .flatten()
                .filter_map(Choice::needs)
                .map(|needs| (format!("{at}.one_of"), needs));
            for (place, needs) in
                std::iter::once((format!("{at}.needs"), &setting.needs)).chain(inner)
            {
                found.extend(needed(
                    service,
                    &place,
                    Some(&setting.setting),
                    needs,
                    &choices,
                ));
            }
        }
        for secret in service.secrets {
            let at = format!("secret {}:{}.needs", service.namespace, secret.id);
            found.extend(needed(service, &at, None, &secret.needs, &choices));
        }
    }
    found
}

/// Every setting reference `services` hold, each with where it is.
fn references<'a>(
    services: &'a [Reading<'a>],
) -> impl Iterator<Item = (String, &'a super::values::Setting)> {
    services.iter().flat_map(|service| {
        service
            .env
            .iter()
            .filter_map(move |(name, value)| match value {
                Value::Setting(setting) => {
                    Some((format!("service {}.env.{name}", service.service), setting))
                }
                _ => None,
            })
    })
}

/// What is wrong with `one_of` as the choices of `setting`, declared at `at`: none at all,
/// or a value listed twice.
fn listed(at: &str, setting: &str, one_of: &[Choice]) -> Vec<(String, String)> {
    let values: Vec<String> = one_of
        .iter()
        .flat_map(Choice::values)
        .map(Constant::text)
        .collect();
    if values.is_empty() {
        return vec![(
            format!("{at}.one_of"),
            format!("lists no value setting {setting} accepts"),
        )];
    }
    values
        .iter()
        .enumerate()
        .filter(|(index, value)| values.iter().take(*index).any(|earlier| earlier == *value))
        .map(|(_, value)| {
            (
                format!("{at}.one_of"),
                format!("lists {value} twice among setting {setting}'s values"),
            )
        })
        .collect()
}

/// What is wrong with `needs`, declared at `at` by a reference to `own`, or by a secret
/// where there is none: a setting `service` does not read, the setting `own` itself, or a
/// value outside the named setting's choices.
fn needed(
    service: &Reading<'_>,
    at: &str,
    own: Option<&str>,
    needs: &Needs,
    choices: &dyn Fn(&str) -> Option<Vec<String>>,
) -> Vec<(String, String)> {
    let reads = |setting: &str| {
        service
            .env
            .values()
            .any(|value| matches!(value, Value::Setting(read) if read.setting == setting))
    };
    let mut found = Vec::new();
    for (setting, values) in needs {
        if own == Some(setting.as_str()) {
            found.push((
                at.to_owned(),
                format!("names setting {setting}, which it reads itself"),
            ));
        } else if !reads(setting) {
            found.push((
                at.to_owned(),
                format!(
                    "names setting {setting}, which service {} does not read",
                    service.service
                ),
            ));
        } else if let Some(held) = choices(setting) {
            found.extend(
                values
                    .iter()
                    .map(Constant::text)
                    .filter(|value| !held.contains(value))
                    .map(|value| {
                        (
                            at.to_owned(),
                            format!(
                                "lists {value} for setting {setting}, which is not one of its \
                                 values: {}",
                                held.join(", ")
                            ),
                        )
                    }),
            );
        }
    }
    found
}

#[cfg(test)]
mod tests;
