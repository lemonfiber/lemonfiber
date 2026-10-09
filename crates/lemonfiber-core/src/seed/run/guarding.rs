//! Turning on the Usenet indexer aggregator's authentication, and keeping it on.
//!
//! An aggregator with no authentication hands its whole configuration — the indexer
//! accounts it holds and their keys among it — to anything that can reach it, and an
//! indexer account is an account somebody paid for. So seeding names an administrator,
//! mints its password, records it, and only then turns authentication on; and an
//! aggregator found running with none is taken on in place, every indexer it holds kept.
//!
//! **Nothing unknown is taken as guarded.** Authentication is lemonfiber's own only where
//! the password it holds opens the configuration and a read presenting nothing is
//! refused; an answer that does not say, a configuration whose indexers cannot be read,
//! and a read that will not settle are each said as not guarded and never recorded.
//!
//! **What lemonfiber turned on is the operator's to turn off.** One found off again,
//! where lemonfiber turned it on, is kept off and said as drift, and a reset is what
//! turns it back on. One whose password lemonfiber does not hold, or that refuses the
//! one it does, is never turned off or given another administrator to get back in.

use std::time::Duration;

use lemonfiber_manifest::ApiKind;

use super::Ctx;
use crate::nzbhydra2::{Credential, Nzbhydra2, Unguarded};
use crate::seed::{State, Wiring};
use crate::wiring::Fillers;

/// The field of the baseline that records lemonfiber turned the authentication on.
pub(super) const FIELD: &str = "authentication";

/// What the baseline records the authentication as.
const BASIC: &str = "basic";

/// How many times the guarded read is asked again while the service restarts.
const READS: u32 = 40;

/// How long between them: the service takes a few seconds to restart, and up to two
/// minutes on a slow machine.
const BETWEEN_READS: Duration = Duration::from_secs(3);

/// The aggregator, as this pass reaches it.
struct Aggregator {
    /// Its id, which the baseline records it under.
    id: String,
    /// What it is called in the report.
    name: String,
    /// A client for it.
    client: Nzbhydra2,
}

/// The aggregator this stack ships, where it runs one this machine can reach.
///
/// The stack's own and never a plugin's: the password this pass mints, keeps and sends
/// is the stack's, and the gate lets no plugin's service naming the same adapter be
/// handed it.
fn aggregator(ctx: &Ctx, fillers: &Fillers) -> Option<Aggregator> {
    let own = fillers
        .speaking(ApiKind::Nzbhydra2)
        .find(|filler| crate::wiring::crosses(crate::wiring::Holder::Stack, filler.holder()))?;
    Some(Aggregator {
        client: Nzbhydra2::new(
            ctx.seams.http.clone(),
            crate::app::targets::loopback(own.published?),
            &own.id,
        ),
        id: own.id.clone(),
        name: own.name.clone(),
    })
}

/// What the report calls this connection.
fn connection(name: &str) -> String {
    format!("{name} authentication")
}

/// Turn the aggregator's authentication on where it has none, and say how it stands.
///
/// Nothing where the stack runs no aggregator this machine can reach.
pub(super) async fn guarded(
    ctx: &Ctx,
    fillers: &Fillers,
    baseline: &mut crate::baseline::Baseline,
) -> Option<Wiring> {
    let aggregator = aggregator(ctx, fillers)?;
    let at = ctx.stamp();
    let state = match aggregator.client.access().await {
        Err(failure) => crate::seed::unreached(&failure),
        Ok(access) if access.auth_configured => held(ctx, &aggregator).await,
        Ok(_) if baseline.expected(&aggregator.id, FIELD).is_some() => {
            let mut wiring = Wiring::settled(connection(&aggregator.name), State::Drifted);
            wiring.escalate(
                exposure(&aggregator.name),
                "run `lemonfiber reset` to turn its authentication back on".to_owned(),
            );
            return Some(wiring);
        }
        Ok(_) if ctx.dry_run => State::WouldWire {
            yours: Some("no authentication".to_owned()),
            ours: None,
        },
        Ok(_) => turned_on(ctx, &aggregator).await,
    };
    if matches!(state, State::Wired | State::AlreadyWired) {
        baseline.record(&aggregator.id, FIELD, BASIC, &at);
    }
    Some(Wiring::settled(connection(&aggregator.name), state))
}

/// Turn the aggregator's authentication back on where lemonfiber turned it on and it has
/// since been turned off — what a reset owes it — or, until `confirm`, say that it would.
///
/// Nothing where there is no such aggregator.
pub(super) async fn put_back(
    ctx: &Ctx,
    fillers: &Fillers,
    baseline: &crate::baseline::Baseline,
    confirm: bool,
) -> Option<Wiring> {
    let aggregator = aggregator(ctx, fillers)?;
    baseline.expected(&aggregator.id, FIELD)?;
    let settled = |state| Some(Wiring::settled(connection(&aggregator.name), state));
    let access = match aggregator.client.access().await {
        Ok(access) => access,
        Err(failure) => return settled(crate::seed::unreached(&failure)),
    };
    // Authentication that is on owes a reset nothing only where it guards: one that
    // still answers anybody is said, never passed over as already done.
    if access.auth_configured {
        return match refusing(&aggregator, State::AlreadyWired).await {
            State::AlreadyWired => None,
            unguarded => settled(unguarded),
        };
    }
    let state = if confirm {
        turned_on(ctx, &aggregator).await
    } else {
        State::WouldWire {
            yours: Some("no authentication".to_owned()),
            ours: None,
        }
    };
    settled(state)
}

/// An aggregator with authentication on: lemonfiber's own where the password it recorded
/// opens it and a read presenting nothing is refused, refused where there is no password
/// recorded or it is refused, and not guarded where anybody is still answered.
async fn held(ctx: &Ctx, aggregator: &Aggregator) -> State {
    let setting = crate::config::NZBHYDRA2_ADMIN_PASSWORD_KEY;
    let Some(password) = crate::app::targets::recorded_secret(ctx, setting) else {
        return State::Refused {
            reason: format!(
                "{name} has authentication lemonfiber did not turn on, and {setting} holds no \
                 password for it, so lemonfiber leaves it as it is. Record its \
                 administrator's password with `lemonfiber config set {setting} <password>`.",
                name = aggregator.name,
            ),
        };
    };
    match aggregator.client.config(Some(admin(&password))).await {
        Ok(_) => refusing(aggregator, State::AlreadyWired).await,
        Err(crate::ports::service::Failure::Unauthorised { .. }) => State::Refused {
            reason: format!(
                "{name} refuses the password {setting} holds, and lemonfiber does not turn its \
                 authentication off or give it another administrator to get back in. Record \
                 the password it takes with `lemonfiber config set {setting} <password>`.",
                name = aggregator.name,
            ),
        },
        Err(failure) => crate::seed::unreached(&failure),
    }
}

/// `guarded` where a read of the aggregator's configuration presenting nothing is refused;
/// otherwise how it is not: answered, or not settled either way.
async fn refusing(aggregator: &Aggregator, guarded: State) -> State {
    match aggregator.client.exposed().await {
        Ok(false) => guarded,
        Ok(true) => State::Failed {
            detail: format!(
                "{} has authentication on and still answers its configuration to a caller \
                 presenting nothing",
                aggregator.name
            ),
        },
        Err(failure) => crate::seed::unreached(&failure),
    }
}

/// Mint the administrator's password, record it, turn authentication on with it, restart
/// the service so it takes effect, and prove it: a read presenting nothing is refused,
/// one presenting the password is not, and every indexer held before is held still.
///
/// A record left behind by authentication that was not turned on is taken back off,
/// because a recorded password is what a later run reads as the authentication being
/// lemonfiber's own.
async fn turned_on(ctx: &Ctx, aggregator: &Aggregator) -> State {
    let client = &aggregator.client;
    let config = match client.config(None).await {
        Ok(config) => config,
        Err(failure) => return crate::seed::unreached(&failure),
    };
    // Every indexer it holds is what the change is proven to keep, so a configuration
    // whose indexers cannot be read is one the change could never be proven on.
    let Some(before) = crate::nzbhydra2::indexers(&config) else {
        return State::Failed {
            detail: format!(
                "{}'s configuration holds no list of indexers to show kept, so its \
                 authentication was not turned on",
                aggregator.name
            ),
        };
    };
    let setting = crate::config::NZBHYDRA2_ADMIN_PASSWORD_KEY;
    let Some(password) = crate::secret::generate(ctx.seams.random.as_ref()) else {
        return State::Failed {
            detail: "no randomness was available to generate a password".to_owned(),
        };
    };
    if let Err(failure) = crate::app::targets::record_secret(ctx, setting, &password) {
        return State::Failed {
            detail: format!(
                "the password lemonfiber generated could not be recorded, so authentication \
                 was not turned on: {failure}"
            ),
        };
    }
    match client.guard(config, admin(&password)).await {
        Ok(()) => {}
        Err(Unguarded::Untaken(failure)) => {
            if let Some(env) = ctx.settings.env_file.as_deref() {
                let _ = crate::config::store::unset(env, setting);
            }
            return crate::seed::unreached(&failure);
        }
        // Sent, and whether it was taken cannot be told: the password stays recorded,
        // because a service that did take it holds it from its next start, and the
        // record is the only copy.
        Err(Unguarded::Unknown(failure)) => {
            return State::Failed {
                detail: format!(
                    "{} was handed its authentication and whether it took it cannot be told, \
                     so the password stays recorded under {setting}: {failure}",
                    aggregator.name
                ),
            };
        }
    }
    // The restart is asked for at once, while the service still answers the way it did
    // before the change: the change takes effect with the restart, and nothing else
    // reaches it until then. The password stays recorded whether or not the restart is
    // taken, because the change is, and the service holds it from its next start.
    if let Err(failure) = client.restart().await {
        return crate::seed::unreached(&failure);
    }
    proven(aggregator, &password, &before).await
}

/// Whether the change took: the configuration refused to a caller presenting nothing
/// once the service is back, read with the password, and holding every indexer it held.
async fn proven(aggregator: &Aggregator, password: &str, before: &[String]) -> State {
    let client = &aggregator.client;
    let mut refused = false;
    for read in 0..READS {
        if matches!(client.exposed().await, Ok(false)) {
            refused = true;
            break;
        }
        if read + 1 < READS {
            tokio::time::sleep(BETWEEN_READS).await;
        }
    }
    if !refused {
        return State::Failed {
            detail: format!(
                "{} took the authentication and still answers its configuration to a caller \
                 presenting nothing; it may not have restarted",
                aggregator.name
            ),
        };
    }
    let after = match client.config(Some(admin(password))).await {
        Ok(config) => crate::nzbhydra2::indexers(&config),
        Err(failure) => return crate::seed::unreached(&failure),
    };
    let Some(after) = after else {
        return State::Failed {
            detail: format!(
                "{}'s configuration, read with the password, holds no list of indexers, so \
                 nothing shows the ones it held were kept",
                aggregator.name
            ),
        };
    };
    let lost: Vec<&str> = before
        .iter()
        .filter(|one| !after.contains(one))
        .map(String::as_str)
        .collect();
    if lost.is_empty() {
        State::Wired
    } else {
        State::Failed {
            detail: format!(
                "{} no longer holds {} after its authentication was turned on",
                aggregator.name,
                lost.join(", ")
            ),
        }
    }
}

/// The administrator lemonfiber names, holding `password`.
fn admin(password: &str) -> Credential<'_> {
    Credential {
        username: crate::config::NZBHYDRA2_ADMIN_USER,
        password,
    }
}

/// What an aggregator with no authentication exposes, in the operator's terms.
pub(crate) fn exposure(name: &str) -> String {
    format!(
        "{name} answers its whole configuration, the indexer accounts it holds and their keys \
         among it, to anything that can reach it"
    )
}
