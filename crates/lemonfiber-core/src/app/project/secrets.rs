//! Each service's own secrets file: every variable its environment names a secret for,
//! with the value read from where it is held, in a file no container mounts.

use lemonfiber_manifest::container::Value;
use lemonfiber_manifest::{Manifest, Service};

use super::super::Ctx;
use crate::config::store::{self, Failure};
use crate::plugin::{Holding, Installed, Variable};
use crate::settings::Resolved;

/// Write the secrets file of every service of the stack and of `installed` whose
/// environment names a secret.
///
/// # Errors
///
/// Where a store cannot be read or a file cannot be written.
pub(crate) fn written(
    ctx: &Ctx,
    manifest: &Manifest,
    installed: &[Installed],
) -> Result<(), Failure> {
    let resolved = Resolved::of(ctx, Some(manifest), installed);
    for service in crate::project::written(manifest)
        .filter(|service| crate::project::names_a_secret(&service.env))
    {
        if let Some(path) = resolved.secrets_file(&service.id) {
            store::write(&path, &crate::settings::text(ctx, &variables(service))?)?;
        }
    }
    let plugins: Vec<&Installed> = installed.iter().collect();
    crate::settings::files::written_by(ctx, &plugins, &resolved)
}

/// Every variable of a stack service's secrets file, each with where its value is read
/// from.
pub(crate) fn variables(service: &Service) -> Vec<Variable> {
    service
        .env
        .iter()
        .filter_map(|(name, value)| match value {
            Value::Secret(secret) => Some(Variable {
                name: name.clone(),
                holding: holding(service, &secret.secret),
            }),
            _ => None,
        })
        .collect()
}

/// Where a stack service's secret is read from: the secret store, where the operator
/// enters it, and otherwise the credential lemonfiber keeps for the service it is of,
/// under the stack's own name for it, read where it is held and never copied.
fn holding(service: &Service, id: &str) -> Holding {
    let named = crate::settings::named(&service.id, id);
    if crate::settings::legacy::named(&named).is_some_and(|row| row.secret) {
        return Holding::Store(named);
    }
    let of = service
        .secret
        .iter()
        .find(|secret| secret.id == id)
        .map_or(service.id.as_str(), |secret| secret.of.as_str());
    Holding::Crossing(crate::config::for_service(
        of,
        crate::config::API_KEY_SUFFIX,
    ))
}
