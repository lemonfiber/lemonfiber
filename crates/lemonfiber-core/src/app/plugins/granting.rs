//! What crosses to a third-party plugin only because the operator grants it (`F14-R20`).

use lemonfiber_manifest::Manifest;

use super::following::diagnosed;
use crate::error::codes::plugin::UNAIMED_CROSSING;
use crate::error::{Problem, Remedy, State};
use crate::journal::{Change, Kind};
use crate::plugin::first_party::{holds, FirstParty};
use crate::plugin::{Grant, Installed};

use super::super::Ctx;

/// Refuse a plugin that is not one of `trusted` and declares a secret of another service
/// that names no capability of its own it is for, before anything is written.
///
/// # Errors
///
/// Where it declares one, naming the secret and whose credential it is.
pub(super) fn aimed(would: &Installed, trusted: &[FirstParty]) -> Result<(), Box<Problem>> {
    if holds(trusted, would) {
        return Ok(());
    }
    crossing(would)
        .find(|secret| secret.serves.is_none())
        .map_or(Ok(()), |secret| {
            Err(Box::new(
                Problem::new(
                    UNAIMED_CROSSING,
                    format!(
                        "{}'s secret {} is {}'s credential and names no capability of {}'s own \
                         it is for",
                        would.plugin, secret.id, secret.of, would.plugin
                    ),
                    "Nothing was written. A third-party plugin is granted another service's \
                     credential only for a capability it fills, one credential for each.",
                    Remedy::new(format!(
                        "Take it up with whoever published {}: the secret names the capability \
                         it is for with for",
                        would.plugin
                    )),
                )
                .in_state(State::Guided),
            ))
        })
}

/// Each grant `would` asks for where it is not one of `trusted`: every secret of another
/// service it declares, by the setting that service's credential is held under here, with
/// the capability it is for. Nothing for a first-party plugin, which `F14-R18` lets the
/// stack's credentials reach whole, and nothing for a service holding no credential here.
pub(super) fn asked(
    ctx: &Ctx,
    stack: &Manifest,
    others: &[Installed],
    would: &Installed,
    trusted: &[FirstParty],
) -> Vec<Grant> {
    if holds(trusted, would) {
        return Vec::new();
    }
    let mut every: Vec<Installed> = others
        .iter()
        .filter(|one| one.plugin != would.plugin)
        .cloned()
        .collect();
    every.push(would.clone());
    let fillers = crate::wiring::Fillers::trusting(
        stack,
        &every,
        &crate::app::targets::chosen_fillers(ctx),
        None,
        trusted,
    );
    let mut asked: Vec<Grant> = Vec::new();
    for secret in crossing(would) {
        let Some(serves) = secret.serves.clone() else {
            continue;
        };
        let Some(credential) = fillers
            .service(&secret.of)
            .filter(|owner| owner.brought_by() != Some(would.plugin.as_str()))
            .and_then(|owner| fillers.setting(owner, crate::config::API_KEY_SUFFIX))
        else {
            continue;
        };
        if asked.iter().all(|one| one.credential != credential) {
            asked.push(Grant {
                credential,
                of: secret.of.clone(),
                serves,
                at: String::new(),
            });
        }
    }
    asked
}

/// Each of `grants` stamped as approved at `stamp`.
pub(super) fn approved(grants: &[Grant], stamp: &str) -> Vec<Grant> {
    grants
        .iter()
        .map(|grant| Grant {
            at: stamp.to_owned(),
            ..grant.clone()
        })
        .collect()
}

/// Journal, under `plugin`'s operation at `stamp`, every grant of `was` that `now` does
/// not keep as ended and every grant of `now` as given.
///
/// # Errors
///
/// Where there is no journal to write to, or it would not take them.
pub(super) fn journalled(
    ctx: &Ctx,
    plugin: &str,
    was: &[Grant],
    now: &[Grant],
    stamp: &str,
) -> Result<(), Box<Problem>> {
    let ended = was
        .iter()
        .filter(|held| !now.contains(held))
        .map(|held| (held, false));
    let given = now
        .iter()
        .filter(|grant| !was.contains(grant))
        .map(|grant| (grant, true));
    let changes: Vec<Change> = ended
        .chain(given)
        .map(|(grant, given)| change(plugin, grant, given, stamp))
        .collect();
    if changes.is_empty() {
        return Ok(());
    }
    let paths = crate::app::targets::layout(ctx)
        .ok_or_else(|| diagnosed(&crate::config::store::Failure::Nowhere))?;
    crate::app::recover::journalled(&paths.journal(), &changes, ctx.seams.random.as_ref())
        .map_err(|failure| diagnosed(&failure))
}

/// The journalled change of `plugin` being given `grant`, or of the grant ending.
pub(crate) fn change(plugin: &str, grant: &Grant, given: bool, stamp: &str) -> Change {
    Change {
        at: stamp.to_owned(),
        operation: crate::plugin::owner(plugin),
        target: grant.credential.clone(),
        kind: Kind::Granted {
            plugin: plugin.to_owned(),
            credential: grant.credential.clone(),
            of: grant.of.clone(),
            serves: grant.serves.clone(),
            at: grant.at.clone(),
            given,
        },
    }
}

/// Every secret of `would` that is another service's credential.
fn crossing(would: &Installed) -> impl Iterator<Item = &crate::plugin::Secret> {
    would.declared.secrets.iter().filter(move |secret| {
        !crate::plugin::own_secret(&would.plugin, crate::plugin::services_of(would), &secret.of)
    })
}

#[cfg(test)]
mod tests;
