//! The secrets lemonfiber mints for a plugin, and the fields an adapter is handed them in.

use lemonfiber_contract::capabilities::owner::claim;
use lemonfiber_contract::spoken;

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

/// Whether each secret stood in for when recorded is one lemonfiber does not mint, and
/// stood in for by a value a reader can see.
pub(super) fn stood_in(manifest: &Manifest, found: &mut Vec<Violation>) {
    for secret in &manifest.secrets {
        let Some(recorded) = &secret.recorded_with else {
            continue;
        };
        let location = format!("secret {}.recorded_with", secret.id);
        if secret.minted.is() {
            found.push(Violation {
                location: location.clone(),
                message: "stands in for a secret lemonfiber mints, and a stand-in is only for a \
                          value the operator alone holds"
                    .to_owned(),
            });
        }
        if let Some(message) = recorded.unshown() {
            found.push(Violation { location, message });
        }
    }
}

/// Whether each secret declaring `lapses_s` is a code a part is claimed with: a positive
/// number of seconds, beside `recorded_with`, neither minted nor carried from a moved
/// service, of a service an adapter speaking `owner.claim@1` fronts and hands it to in its
/// `upstream`.
pub(super) fn lapsing(manifest: &Manifest, found: &mut Vec<Violation>) {
    for secret in &manifest.secrets {
        let Some(lapses) = secret.lapses_s else {
            continue;
        };
        let location = format!("secret {}.lapses_s", secret.id);
        let mut refused = |message: &str| {
            found.push(Violation {
                location: location.clone(),
                message: message.to_owned(),
            });
        };
        if lapses == 0 {
            refused("is no seconds at all, and a code that lapses holds for some");
        }
        if secret.recorded_with.is_none() {
            refused("is declared without recorded_with, the stand-in plugin record hands it");
        }
        if secret.minted.is() || secret.carried_from.is_some() {
            refused(
                "is declared on a secret lemonfiber mints or carries, and a code that lapses is \
                 one the operator alone enters",
            );
        }
        let claim = spoken(claim::CAPABILITY, claim::MAJOR);
        let claimed = manifest.services.iter().any(|service| {
            service.fronts.as_deref() == Some(secret.of.as_str())
                && service.speaks.contains(&claim)
                && service
                    .upstream
                    .values()
                    .any(|handed| handed.secret == secret.id)
        });
        if !claimed {
            refused(&format!(
                "names {}, which no adapter speaking {claim} fronts and hands it to in its upstream",
                secret.of
            ));
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
