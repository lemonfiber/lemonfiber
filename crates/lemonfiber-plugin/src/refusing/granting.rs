//! What a credential crossing to a plugin is for (`ARCH-R322`).

use crate::schema::Manifest;
use crate::Violation;

/// Whether each secret naming `for` is of a service outside the plugin, names a capability
/// one of its own services provides, and is the only such secret for that capability.
pub(super) fn aimed(manifest: &Manifest, found: &mut Vec<Violation>) {
    let provided: Vec<&str> = manifest
        .services
        .iter()
        .flat_map(|service| service.provides.iter().map(String::as_str))
        .collect();
    let mut taken: Vec<&str> = Vec::new();
    for secret in &manifest.secrets {
        let Some(serves) = secret.serves.as_deref() else {
            continue;
        };
        let location = format!("secret {}.for", secret.id);
        let message = if own(manifest, &secret.of) {
            format!(
                "names {serves}, and {} is this plugin's own, so nothing crosses for it to fill",
                secret.of
            )
        } else if !provided.contains(&serves) {
            format!(
                "names {serves}, which none of this plugin's services provides; it provides {}",
                if provided.is_empty() {
                    "nothing".to_owned()
                } else {
                    provided.join(", ")
                }
            )
        } else if taken.contains(&serves) {
            format!(
                "names {serves}, which another secret crossing to this plugin is for, and one \
                 credential crosses for each capability it fills"
            )
        } else {
            taken.push(serves);
            continue;
        };
        found.push(Violation { location, message });
    }
}

/// Whether `of` names the plugin itself or one of its services.
fn own(manifest: &Manifest, of: &str) -> bool {
    manifest.plugin.id == of || manifest.services.iter().any(|service| service.id == of)
}

#[cfg(test)]
mod tests;
