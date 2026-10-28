//! The values a plugin's settings accept and the values of other settings each reference
//! and secret is read under, held to the rules every manifest is.

use lemonfiber_manifest::container::choices::{self, Reading};

use crate::schema::Manifest;
use crate::Violation;

/// Every rule what the manifest declares about choices and needs breaks.
pub(super) fn chosen(manifest: &Manifest, found: &mut Vec<Violation>) {
    let secrets: Vec<Vec<&lemonfiber_manifest::container::Secret>> = manifest
        .services
        .iter()
        .map(|service| {
            manifest
                .secrets
                .iter()
                .filter(|secret| secret.of == service.id)
                .collect()
        })
        .collect();
    let readings: Vec<Reading<'_>> = manifest
        .services
        .iter()
        .zip(&secrets)
        .map(|(service, secrets)| Reading {
            service: &service.id,
            env: &service.env,
            namespace: &manifest.plugin.id,
            secrets,
        })
        .collect();
    found.extend(
        choices::broken(&readings)
            .into_iter()
            .map(|(location, message)| Violation { location, message }),
    );
}

#[cfg(test)]
mod tests;
