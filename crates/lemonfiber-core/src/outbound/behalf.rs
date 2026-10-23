//! The requests lemonfiber makes on an installed plugin's behalf, attributed to it.

use lemonfiber_manifest::Service;

use super::OnBehalf;

/// What is said about a destination a plugin's recipes declare.
pub(super) const CARRIED: &str = "A destination this plugin's recipes declare they may call, or \
                                  carry a value it captured to. Declared in its manifest and \
                                  held to it: a recipe reaching anywhere else is refused before \
                                  it is installed.";

/// Every destination outside the stack an installed plugin's recipes declare, each once
/// per plugin and attributed to it.
///
/// A recipe reaching one of the stack's own services is not something leaving the
/// machine, so it is not listed.
pub(super) fn declared(stack: &[Service], installed: &[crate::plugin::Installed]) -> Vec<OnBehalf> {
    let inside = |to: &str| stack.iter().any(|service| service.id == to);
    installed
        .iter()
        .flat_map(|one| {
            one.declared
                .reaches
                .iter()
                .filter(|to| !inside(to))
                .map(|to| OnBehalf {
                    plugin: one.plugin.clone(),
                    destination: to.clone(),
                    purpose: CARRIED.to_owned(),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests;
