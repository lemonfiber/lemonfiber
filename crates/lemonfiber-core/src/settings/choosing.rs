//! A change held to each setting's choices: every setting it names, and every other whose
//! choices hang on one it names, has to be left holding one of them.

use std::collections::BTreeMap;

use lemonfiber_manifest::container::{choices, Value};
use lemonfiber_manifest::Manifest;

use crate::error::codes::setting::OUTSIDE_ITS_CHOICES;
use crate::error::{Problem, Remedy, State};
use crate::plugin::Installed;

/// One setting a change would leave outside its choices.
#[derive(Debug)]
pub(super) struct Outside {
    /// The setting.
    setting: String,
    /// The value it would hold.
    value: String,
    /// The values it would accept.
    choices: Vec<String>,
}

/// Every setting `changing` would leave holding a value outside its choices, where
/// `recorded` says what each holds now: each setting it names, and each other recorded
/// one whose choices need one it names.
pub(super) fn outside(
    manifest: &Manifest,
    installed: &[Installed],
    changing: &BTreeMap<&str, Option<&str>>,
    recorded: &dyn Fn(&str) -> Option<String>,
) -> Vec<Outside> {
    let after = |name: &str| match changing.get(name) {
        Some(value) => value.map(str::to_owned),
        None => recorded(name),
    };
    let found: BTreeMap<String, Outside> = environments(manifest, installed)
        .flat_map(|env| {
            env.values()
                .filter_map(|value| match value {
                    Value::Setting(read) => Some((read, read.one_of.as_deref()?)),
                    _ => None,
                })
                .filter(|(read, one_of)| touched(&read.setting, one_of, changing))
                .filter_map(|(read, one_of)| {
                    let value = after(&read.setting)?;
                    let held = |name: &str| after(name).or_else(|| choices::defaulted(env, name));
                    let choices = choices::open(one_of, &held);
                    (!choices.contains(&value)).then(|| {
                        (
                            read.setting.clone(),
                            Outside {
                                setting: read.setting.clone(),
                                value,
                                choices,
                            },
                        )
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect();
    found.into_values().collect()
}

/// Whether `changing` reaches the choices `one_of` declares for `setting`: it names the
/// setting, or one their needs name.
fn touched(
    setting: &str,
    one_of: &[choices::Choice],
    changing: &BTreeMap<&str, Option<&str>>,
) -> bool {
    changing.contains_key(setting)
        || one_of
            .iter()
            .filter_map(choices::Choice::needs)
            .flat_map(|needs| needs.keys())
            .any(|name| changing.contains_key(name.as_str()))
}

/// Every service's environment: the stack's own, then each installed plugin's.
fn environments<'a>(
    manifest: &'a Manifest,
    installed: &'a [Installed],
) -> impl Iterator<Item = &'a BTreeMap<String, Value>> {
    manifest
        .services
        .iter()
        .map(|service| service.container().env)
        .chain(
            installed
                .iter()
                .flat_map(|one| one.services.iter().map(|placed| &placed.fields.env)),
        )
}

/// The refusal naming each setting `outside` lists, the value it would hold and the
/// choices it has.
pub(super) fn refused(outside: &[Outside]) -> Problem {
    let named: Vec<String> = outside
        .iter()
        .map(|one| {
            format!(
                "{} cannot hold {}; its choices are {}",
                one.setting,
                one.value,
                if one.choices.is_empty() {
                    "none, given what the settings it depends on hold".to_owned()
                } else {
                    one.choices.join(", ")
                }
            )
        })
        .collect();
    let settings: Vec<&str> = outside.iter().map(|one| one.setting.as_str()).collect();
    Problem::new(
        OUTSIDE_ITS_CHOICES,
        format!(
            "{} would hold a value outside its choices",
            settings.join(" and ")
        ),
        format!("{}. Nothing was changed.", named.join(". ")),
        Remedy::new("Give one of the choices, moving the settings it depends on in the same act")
            .with_detail("lemonfiber settings set <setting>=<value> <setting>=<value>"),
    )
    .in_state(State::Guided)
}
