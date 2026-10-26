//! The secrets lemonfiber mints for a plugin, and the fields an adapter is handed them in.

use crate::schema::{Manifest, Service};
use crate::seeding::owned;
use crate::Violation;

/// Whether every minted secret is one of the plugin's own services' and nothing captures
/// it.
pub(super) fn minted(manifest: &Manifest, found: &mut Vec<Violation>) {
    for secret in manifest.secrets.iter().filter(|secret| secret.minted.is()) {
        let at = format!("secret {}", secret.id);
        if !manifest
            .services
            .iter()
            .any(|service| service.id == secret.of)
        {
            found.push(Violation {
                location: format!("{at}.of"),
                message: format!(
                    "names {}, and a secret lemonfiber mints is the credential of one of this \
                     plugin's own services",
                    secret.of
                ),
            });
        }
        let captured = manifest
            .recipes
            .iter()
            .flat_map(|recipe| recipe.steps.iter())
            .any(|step| step.capture.iter().any(|capture| capture.name == secret.id));
        if captured {
            found.push(Violation {
                location: at,
                message: "is minted and is also captured by a recipe, and a value has one source"
                    .to_owned(),
            });
        }
    }
}

/// Whether a service's `upstream` is an adapter's, each field written as a field of
/// `upstream.json` is and each naming one of the plugin's own secrets.
pub(super) fn handed(manifest: &Manifest, service: &Service, found: &mut Vec<Violation>) {
    if service.upstream.is_empty() {
        return;
    }
    let at = format!("service {}.upstream", service.id);
    if service.speaks.is_empty() {
        found.push(Violation {
            location: at.clone(),
            message: "hands an upstream its credentials, and only a service that speaks a \
                      contract is an adapter with an upstream"
                .to_owned(),
        });
    }
    for (field, secret) in &service.upstream {
        if !is_field(field) {
            found.push(Violation {
                location: format!("{at}.{field}"),
                message: "is not lowercase letters, digits and _ starting with a letter".to_owned(),
            });
        }
        if let Err(message) = owned(manifest, &secret.secret) {
            found.push(Violation {
                location: format!("{at}.{field}"),
                message,
            });
        }
    }
}

/// Whether `field` is lowercase letters, digits and `_`, starting with a letter.
fn is_field(field: &str) -> bool {
    field
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && field
            .chars()
            .all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '_')
}

#[cfg(test)]
mod tests;
