//! The variables an installed plugin's settings and secrets name in `was`.

use lemonfiber_manifest::container::earlier::{self, Earlier};

use super::Installed;

/// Every variable `one`'s settings and secrets name in `was`.
#[must_use]
pub fn of(one: &Installed) -> Vec<Earlier> {
    earlier::distinct(
        earlier::settings(one.services.iter().map(|placed| &placed.fields.env))
            .into_iter()
            .chain(one.declared.secrets.iter().filter_map(|secret| {
                earlier::secret(&one.plugin, &secret.id, secret.was.as_deref())
            })),
    )
}

/// The first variable `would` names in `was` that another installed plugin names for
/// another setting or secret: both, and whose the other is.
#[must_use]
pub fn taken(would: &Installed, installed: &[Installed]) -> Option<(Earlier, Earlier, String)> {
    let own = of(would);
    installed
        .iter()
        .filter(|other| other.plugin != would.plugin)
        .find_map(|other| {
            let theirs = of(other);
            earlier::clashing(&own, &theirs)
                .first()
                .map(|(one, held)| ((*one).clone(), (*held).clone(), other.plugin.clone()))
        })
}

#[cfg(test)]
mod tests;
