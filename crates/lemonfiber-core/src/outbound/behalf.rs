//! The requests lemonfiber makes on an installed plugin's behalf, attributed to it, each
//! refused by itself under a setting of its own.

use lemonfiber_manifest::Service;

use super::OnBehalf;
use crate::plugin::{Installed, Stopping};

/// What is said about a destination a plugin's recipes declare.
pub(super) const CARRIED: &str = "A destination this plugin's recipes declare they may call, or \
                                  carry a value it captured to. Declared in its manifest and \
                                  held to it: a recipe reaching anywhere else is refused before \
                                  it is installed.";

/// What every setting refusing a plugin's reach begins with, before the plugin's id.
const PLUGIN: &str = "plugin.";

/// What stands between the plugin's id and the host in a setting refusing its reach.
const REACH: &str = ".reach.";

/// The values a setting refusing a plugin's reach takes: allowed, then refused.
pub(crate) const CHOICES: [&str; 2] = ["on", "off"];

/// The setting refusing `plugin` the host `host`.
#[must_use]
pub fn switch(plugin: &str, host: &str) -> String {
    format!("{PLUGIN}{plugin}{REACH}{host}")
}

/// The plugin and the host a setting refusing a plugin's reach names, where `name` is one.
#[must_use]
pub(crate) fn switched(name: &str) -> Option<(&str, &str)> {
    name.strip_prefix(PLUGIN)?
        .split_once(REACH)
        .filter(|(plugin, host)| !plugin.is_empty() && !host.is_empty())
}

/// Every host outside the stack `one`'s recipes reach.
pub(crate) fn hosts<'a>(stack: &'a [Service], one: &'a Installed) -> impl Iterator<Item = &'a str> {
    one.declared
        .reaches
        .iter()
        .map(String::as_str)
        .filter(|to| !stack.iter().any(|service| service.id == *to))
}

/// Every setting refusing one of `installed`'s hosts outside the stack.
pub(crate) fn switches(stack: &[Service], installed: &[Installed]) -> Vec<String> {
    installed
        .iter()
        .flat_map(|one| hosts(stack, one).map(|host| switch(&one.plugin, host)))
        .collect()
}

/// Whether `value`, held by a setting refusing a plugin's reach, refuses it.
fn refuses(value: Option<&str>) -> bool {
    value.is_some_and(crate::config::reads_as_off)
}

/// Every host outside the stack the operator refused `one`, as `recorded` says what each
/// setting holds.
pub(crate) fn refused(
    stack: &[Service],
    one: &Installed,
    recorded: &dyn Fn(&str) -> Option<String>,
) -> Vec<String> {
    hosts(stack, one)
        .filter(|host| refuses(recorded(&switch(&one.plugin, host)).as_deref()))
        .map(str::to_owned)
        .collect()
}

/// What refusing a host stops, as an operator reads it: its author's line where the
/// manifest gives one, then every recipe step it stops.
pub(crate) fn cost(stopping: Option<&Stopping>) -> String {
    let Some(stopping) = stopping else {
        return "What refusing it stops was not recorded when this plugin was installed; \
                installing it again records it."
            .to_owned();
    };
    let mut recipes: Vec<(&str, Vec<&str>)> = Vec::new();
    for stopped in &stopping.steps {
        match recipes
            .iter_mut()
            .find(|(title, _)| *title == stopped.title)
        {
            Some((_, steps)) => steps.push(&stopped.step),
            None => recipes.push((&stopped.title, vec![&stopped.step])),
        }
    }
    let steps = if recipes.is_empty() {
        "No step of its recipes calls it, so refusing it stops none.".to_owned()
    } else {
        format!(
            "Not asked: {}.",
            recipes
                .iter()
                .map(|(title, steps)| format!("{title} ({})", steps.join(", ")))
                .collect::<Vec<_>>()
                .join("; ")
        )
    };
    match &stopping.said {
        Some(said) => format!("{said} {steps}"),
        None => steps,
    }
}

/// What setting the reach switch `name` to `value` does: what refusing it stops where it
/// refuses, and that the host may be asked again where it does not; nothing where no
/// installed plugin reaches the host it names.
pub(crate) fn consequence(
    installed: &[Installed],
    name: &str,
    value: Option<&str>,
) -> Option<String> {
    let (plugin, host) = switched(name)?;
    let one = installed.iter().find(|one| one.plugin == plugin)?;
    if !one.declared.reaches.iter().any(|to| to == host) {
        return None;
    }
    if refuses(value) {
        return Some(cost(
            one.declared
                .stops
                .iter()
                .find(|stopping| stopping.host == host),
        ));
    }
    Some(format!("{plugin}'s recipes may ask {host} again."))
}

/// Every destination outside the stack an installed plugin's recipes declare, each once
/// per plugin and attributed to it, with whether `recorded` allows it, the setting that
/// refuses it and what refusing it stops.
///
/// A recipe reaching one of the stack's own services is not something leaving the
/// machine, so it is not listed.
pub(super) fn declared(
    stack: &[Service],
    installed: &[Installed],
    recorded: &dyn Fn(&str) -> Option<String>,
) -> Vec<OnBehalf> {
    installed
        .iter()
        .flat_map(|one| {
            hosts(stack, one).map(move |to| {
                let switch = switch(&one.plugin, to);
                OnBehalf {
                    plugin: one.plugin.clone(),
                    destination: to.to_owned(),
                    purpose: CARRIED.to_owned(),
                    allowed: !refuses(recorded(&switch).as_deref()),
                    switch,
                    cost: cost(
                        one.declared
                            .stops
                            .iter()
                            .find(|stopping| stopping.host == to),
                    ),
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
