//! The credentials a service reads out of another service's.
//!
//! A service naming another's credential among its secrets — the quality sync and the
//! archive extractor each name every curator's they work with — is handed it in its
//! secrets file, read from where its holder keeps it each time the file is written. Such a
//! service speaks no contract, so the connection table pairs it with nothing it reads, and
//! this pass is where one it is handed none of is said.
//!
//! **Said where it is not handed, never where it is.** A credential the gate does not let
//! cross is unmatched, since no run changes that; one nothing holds yet is skipped, since a
//! later run completes it. A credential of a service that is not here is left to what the
//! stack reports filling nothing.

use lemonfiber_manifest::container::Value;
use lemonfiber_manifest::Manifest;

use super::Ctx;
use crate::plugin::{Holding, Installed};
use crate::seed::{State, Wiring};
use crate::settings::Resolved;
use crate::wiring::{Filler, Fillers, Holder};

/// Every credential a service of the stack or of `installed` reads out of another's and
/// is handed none of, said naming both.
pub(super) fn unhanded(
    ctx: &Ctx,
    manifest: &Manifest,
    installed: &[Installed],
    fillers: &Fillers,
) -> Vec<Wiring> {
    let resolved = Resolved::of(ctx, Some(manifest), installed);
    let mut said = Vec::new();
    for (reader, of, holding) in read(manifest, installed, &resolved) {
        let (Some(owner), Some(recipient)) = (fillers.service(&of), fillers.service(&reader))
        else {
            continue;
        };
        let state = match holding {
            Holding::Withheld => State::Unmatched {
                reason: withheld(owner, recipient),
            },
            Holding::Store(name) if crate::app::targets::stored_secret(ctx, &name).is_none() => {
                unheld(owner)
            }
            Holding::Crossing(setting)
                if crate::app::targets::recorded_secret(ctx, &setting).is_none() =>
            {
                unheld(owner)
            }
            Holding::Store(_) | Holding::Crossing(_) => continue,
        };
        said.push(Wiring::settled(
            format!("{}'s key into {}", owner.name, recipient.name),
            state,
        ));
    }
    said
}

/// Each credential a service the project writes reads out of another's: the service
/// reading it, the service it is of, and where it is read from.
fn read(
    manifest: &Manifest,
    installed: &[Installed],
    resolved: &Resolved,
) -> Vec<(String, String, Holding)> {
    let stack = crate::project::written(manifest, installed).flat_map(|service| {
        service
            .secret
            .iter()
            .filter(|secret| secret.of != service.id && names(&service.env, &secret.id))
            .map(|secret| {
                (
                    service.id.clone(),
                    secret.of.clone(),
                    crate::settings::files::stack_holding(service, &secret.id, resolved),
                )
            })
    });
    let brought = installed.iter().flat_map(|plugin| {
        plugin
            .declared
            .secrets
            .iter()
            .filter(|secret| {
                !crate::plugin::own_secret(
                    &plugin.plugin,
                    crate::plugin::services_of(plugin),
                    &secret.of,
                )
            })
            .flat_map(move |secret| {
                plugin
                    .services
                    .iter()
                    .filter(|placed| names(&placed.fields.env, &secret.id))
                    .map(move |placed| {
                        (
                            placed.service.clone(),
                            secret.of.clone(),
                            resolved.reading(|reading| {
                                crate::plugin::holding(plugin, &secret.id, reading)
                            }),
                        )
                    })
            })
    });
    stack.chain(brought).collect()
}

/// Whether an environment names the secret `id`.
fn names(env: &std::collections::BTreeMap<String, Value>, id: &str) -> bool {
    env.values()
        .any(|value| matches!(value, Value::Secret(secret) if secret.secret == id))
}

/// Why `owner`'s credential is not handed to `recipient`, in the operator's terms.
fn withheld(owner: &Filler, recipient: &Filler) -> String {
    let (owner, reads) = (
        &owner.name,
        format!("{} reads {}'s key", recipient.name, owner.name),
    );
    match recipient.holder() {
        Holder::ThirdParty(_) => format!(
            "{reads}, and is a third-party plugin's service, which is handed another service's \
             credential only where the operator granted it in an install or update"
        ),
        Holder::Stack | Holder::FirstParty(_) | Holder::Nobody => format!(
            "{reads}, and {owner} is a third-party plugin's service, whose credential is handed \
             only to the stack's own services and to its own plugin's"
        ),
    }
}

/// What a credential nothing holds yet comes to.
fn unheld(owner: &Filler) -> State {
    State::Skipped {
        reason: format!(
            "nothing holds {}'s key yet; a later run completes it",
            owner.name
        ),
    }
}

#[cfg(test)]
mod tests;
