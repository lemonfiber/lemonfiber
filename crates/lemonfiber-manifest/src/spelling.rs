//! How a service's id is written where only an environment name may go.
//!
//! lemonfiber keeps what a service holds — its published key, the password it minted
//! for it — in settings named after the service. A service id is a Compose name and may
//! carry hyphens, which a shell reads as an operator, so the name is the id upper-cased
//! with anything else replaced.
//!
//! Here, below both manifests, because two ids that are written alike this way share a
//! setting, and refusing that is a question asked of the stack's ids and a plugin's at
//! once: the plugin's reader, which cannot see the core, and the core, which keeps the
//! settings, have to agree on the spelling or one of them is guarding the wrong name.

/// The id as an environment name spells it: upper-cased, with everything but a letter
/// or a digit written as `_`.
#[must_use]
pub fn environment_name(id: &str) -> String {
    id.to_uppercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests;
