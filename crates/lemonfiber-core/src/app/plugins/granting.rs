//! What crosses to a third-party plugin only because the operator grants it.

use lemonfiber_manifest::container::Value;
use lemonfiber_manifest::Manifest;

use super::following::diagnosed;
use crate::credential::{Granted, Revoked};
use crate::error::codes::plugin::{NO_GRANT, UNAIMED_CROSSING};
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
pub(crate) fn aimed(would: &Installed, trusted: &[FirstParty]) -> Result<(), Box<Problem>> {
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
                        would.plugin,
                        crate::text::plain(&secret.id),
                        crate::text::plain(&secret.of),
                        would.plugin
                    ),
                    "Nothing was written. A third-party plugin is granted another service's \
                     credential only for a capability it fills, one credential for each.",
                    Remedy::new(format!(
                        "Take it up with whoever published {}: each secret of another service \
                         names the capability of the plugin's own it is needed for",
                        would.plugin
                    )),
                )
                .in_state(State::Guided),
            ))
        })
}

/// Each grant `would` asks for where it is not one of `trusted`: every secret of another
/// service it declares, by the setting that service's credential is held under here, with
/// the capability it is for. Nothing for a first-party plugin, which the stack's
/// credentials reach whole, and nothing for a service holding no credential here.
pub(crate) fn asked(
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
pub(crate) fn approved(grants: &[Grant], stamp: &str) -> Vec<Grant> {
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
pub(crate) fn journalled(
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
fn change(plugin: &str, grant: &Grant, given: bool, stamp: &str) -> Change {
    Change {
        at: stamp.to_owned(),
        operation: crate::plugin::owner(plugin),
        target: grant.credential.clone(),
        kind: Kind::Granted {
            plugin: plugin.to_owned(),
            grant: grant.clone(),
            given,
        },
    }
}

/// The parts revoking a grant is offered over, as a refusal names them.
pub(super) const REVOKING: [&str; 2] = ["the grant it ends", "what it would restart"];

/// End `plugin`'s grant of `credential`, or say what ending it would do where no offer
/// is answered or this is a rehearsal.
///
/// The credential is written out of every secrets file that read it and the plugin's
/// services reading it restart, journalled as one operation an undo reverses. What the
/// plugin was handed while it held the grant it may have kept, which only replacing the
/// credential takes back.
///
/// # Errors
///
/// Where the record cannot be read, the plugin holds no such grant, the offer moved, or
/// the journal, the record or a secrets file cannot be written.
pub(crate) async fn revoke(
    ctx: &Ctx,
    plugin: &str,
    credential: &str,
    agreement: Option<&str>,
) -> Result<Revoked, Box<Problem>> {
    let held = super::read(ctx)?;
    let one = held.holds(plugin);
    let grant = one.and_then(|one| {
        one.granted
            .iter()
            .find(|grant| grant.credential.eq_ignore_ascii_case(credential.trim()))
    });
    let (Some(one), Some(grant)) = (one, grant) else {
        return Err(Box::new(no_grant(plugin, credential, one)));
    };
    let restarts = readers(one, &grant.of);
    let offer = crate::agreement::parted(&[
        &[&serde_json::to_string(grant).unwrap_or_default()],
        &[&restarts.join("\n")],
    ]);
    let consent = super::Consent {
        agreement: agreement.map(str::to_owned),
        ..super::Consent::default()
    };
    let mut revoked = Revoked {
        grant: Granted::of(plugin, grant),
        restarts,
        agreement: offer.clone(),
        ended: false,
        unrestarted: None,
    };
    if !super::offering::acting(ctx, &consent, plugin, &offer, &REVOKING, &[])? {
        return Ok(revoked);
    }
    let stamp = ctx.stamp();
    journalled(ctx, plugin, std::slice::from_ref(grant), &[], &stamp)?;
    let mut after = held.clone();
    after.regrant(plugin, grant, false);
    super::super::record::keep(super::kept_at(ctx).as_deref(), &after)?;
    crate::settings::files::reread_now(ctx, &crate::plugin::Holding::Withheld)?;
    let running = crate::settings::running(ctx, &revoked.restarts).await;
    revoked.unrestarted = crate::settings::restarted(ctx, &running).await;
    revoked.ended = true;
    Ok(revoked)
}

/// Each service of `one` whose environment reads a secret of `of`.
fn readers(one: &Installed, of: &str) -> Vec<String> {
    let crossing: Vec<&str> = one
        .declared
        .secrets
        .iter()
        .filter(|secret| secret.of == of)
        .map(|secret| secret.id.as_str())
        .collect();
    one.services
        .iter()
        .filter(|placed| {
            placed.fields.env.values().any(|value| {
                matches!(value, Value::Secret(named) if crossing.contains(&named.secret.as_str()))
            })
        })
        .map(|placed| placed.service.clone())
        .collect()
}

/// A revoke naming a grant `plugin` does not hold, with the grants it does.
fn no_grant(plugin: &str, credential: &str, one: Option<&Installed>) -> Problem {
    let holds: Vec<&str> = one
        .iter()
        .flat_map(|one| one.granted.iter().map(|grant| grant.credential.as_str()))
        .collect();
    Problem::new(
        NO_GRANT,
        format!(
            "{} holds no grant of {}",
            crate::text::plain(plugin),
            crate::text::plain(credential)
        ),
        "Nothing was revoked.",
        Remedy::new(
            "Revoke one of the grants the plugin holds, as lemonfiber credentials lists them",
        ),
    )
    .with_detail(if holds.is_empty() {
        "it holds none".to_owned()
    } else {
        format!("it holds {}", holds.join(", "))
    })
    .in_state(State::Guided)
}

/// Every secret of `would` that is another service's credential.
fn crossing(would: &Installed) -> impl Iterator<Item = &crate::plugin::Secret> {
    would.declared.secrets.iter().filter(move |secret| {
        !crate::plugin::own_secret(&would.plugin, crate::plugin::services_of(would), &secret.of)
    })
}
