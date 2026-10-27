//! The variables an earlier release kept a value under, as a plugin's settings and
//! secrets name them in `was`.

use lemonfiber_manifest::container::earlier::{self, Earlier};

use crate::schema::Manifest;
use crate::Violation;

use super::bundled;

/// Every variable an earlier release kept a value under that the manifest names in `was`
/// and may not: one that is no variable's name, one two of its settings or secrets name,
/// and one the shipped stack names for another.
pub(super) fn named(manifest: &Manifest, found: &mut Vec<Violation>) {
    let stack = earlier::distinct(
        bundled::services()
            .iter()
            .flat_map(|service| earlier::of([&service.env], &service.id, &service.secret)),
    );
    against(manifest, &stack, found);
}

fn against(manifest: &Manifest, stack: &[Earlier], found: &mut Vec<Violation>) {
    let own = earlier::of(
        manifest.services.iter().map(|service| &service.env),
        &manifest.plugin.id,
        &manifest.secrets,
    );
    let clashes = earlier::clashing(&own, stack)
        .into_iter()
        .map(|(one, other)| {
            (
                format!("{}.was", one.named()),
                format!("{} is named by the stack's {}", one.key, other.named()),
            )
        });
    found.extend(
        earlier::broken(&own)
            .into_iter()
            .chain(clashes)
            .map(|(location, message)| Violation { location, message }),
    );
}

#[cfg(test)]
mod tests;
