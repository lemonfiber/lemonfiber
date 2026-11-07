//! The request gate's key on each curator's adapter, as the inventory lists and replaces
//! it: one line per curator the gate routes to.
//!
//! The key is written beside the adapter's configuration, which is the one copy lemonfiber
//! keeps, and the gate presents it on the route named for the media the curator files. It
//! opens only the operations a curator answers the gate.
//!
//! **A replacement keeps a working key at every moment.** A new key is written beside the
//! old, so the adapter admits both, and handed to the gate with every route it admits; the
//! request service's own test of the route through the gate proves it, and only then is the
//! old key taken out. Where any step fails the old routes are handed back and the new key
//! taken out again.

use lemonfiber_manifest::Service;

use super::rotating::{unproven, would_rotate};
use crate::app::gating;
use crate::app::Ctx;
use crate::credential::{fingerprint, Held, Origin, Propagation, Reach, Rotation, State};
use crate::ports::service::{Admitted, Gated, Requests};
use crate::seed::run::request_keys::Keys;
use crate::seed::run::tokens::{through_the_gate, Kept};
use crate::wiring::{Filler, Fillers};

/// What each key is recorded as, before the route it is presented on.
const SETTING: &str = "request-gate/request.key#";

/// What a confirmed ask to print one is told.
pub(super) const UNPRINTED: &str = "this key opens only what the request gate asks a curator, \
                                    and only the gate and the curator's adapter hold it";

/// What a rehearsal says a replacement would take.
const ROTATING: &str = "a real run would write a new key beside the old one on the curator's \
                        adapter, hand it to the request gate, prove it with the request \
                        service's own test through the gate, and only then take the old one \
                        out. Nothing was written here.";

/// Why nothing was written where the gate's media server key could not be read.
const UNREAD_MEDIA: &str = "the request gate's key on the media server could not be read, and \
                            the gate is handed every route at once, so nothing was written";

/// What a landed replacement says.
const LANDED: &str = "the request gate presents the new key, and the curator's adapter admits it \
                      alone";

/// Whether `held` is one of the gate's keys on a curator's adapter.
pub(super) fn is_request_key(held: &Held) -> bool {
    held.setting.starts_with(SETTING)
}

/// One curator the gate routes to, as the inventory names it.
struct Routed<'a> {
    /// The route it answers under.
    route: &'static str,
    /// The curator.
    curator: &'a Filler,
    /// The service speaking `library.curate` for it, which holds the key.
    adapter: &'a Filler,
}

/// Every curator the gate routes to.
fn routed(fillers: &Fillers) -> Vec<Routed<'_>> {
    crate::seed::run::gate::routed_curators(fillers)
        .into_iter()
        .map(|(route, curator, adapter)| Routed {
            route,
            curator,
            adapter,
        })
        .collect()
}

/// A key's line in the inventory, for each curator the gate routes to.
pub(super) async fn held(ctx: &Ctx, services: &[Service], fillers: &Fillers) -> Vec<Held> {
    if gating::service(services).is_none() {
        return Vec::new();
    }
    let routed = routed(fillers);
    if routed.is_empty() {
        return Vec::new();
    }
    let admitted = match Kept::read(ctx, fillers).await {
        Some(kept) => kept.admitted().to_vec(),
        None => Vec::new(),
    };
    routed.iter().map(|one| line(ctx, one, &admitted)).collect()
}

/// The line for `one`, from the key written on its adapter that the gate presents.
fn line(ctx: &Ctx, one: &Routed<'_>, admitted: &[Admitted]) -> Held {
    let presented = admitted
        .iter()
        .find(|route| route.path == one.route)
        .map(|route| route.credential.as_str());
    let (state, key, advisory) = match Keys::of(ctx, one.adapter) {
        Err(why) => (State::Stale, None, Some(why)),
        Ok(keys) => match presented.and_then(|presented| keys.presented(presented)) {
            Some(key) => (State::Active, Some(key.to_owned()), None),
            None => (
                State::Absent,
                None,
                Some(format!(
                    "the request gate presents no key written on {}. Run lemonfiber seed, \
                     which writes one.",
                    one.adapter.name
                )),
            ),
        },
    };
    Held {
        name: name(one),
        setting: format!("{SETTING}{}", one.route),
        consumers: vec![consumer(one)],
        location: format!(
            "beside {}'s configuration, where it reads it",
            one.adapter.name
        ),
        origin: Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state,
        fingerprint: key.as_deref().map(fingerprint),
        advisory,
    }
}

/// Replace the key one line names, keeping a working one at every moment.
pub(super) async fn rotate(ctx: &Ctx, held: &Held, fillers: &Fillers) -> Rotation {
    let route = held.setting.trim_start_matches(SETTING);
    let routed = routed(fillers);
    let Some(one) = routed.iter().find(|one| one.route == route) else {
        return unproven(held, &not_routed(&held.name));
    };
    let Some(kept) = Kept::read(ctx, fillers).await else {
        return unproven(held, &not_routed(&held.name));
    };
    let admitted = kept.admitted().to_vec();
    let mut keys = match Keys::of(ctx, one.adapter) {
        Ok(keys) => keys,
        Err(why) => return unproven(held, &why),
    };
    let Some(old) = admitted
        .iter()
        .find(|presented| presented.path == one.route)
        .and_then(|presented| keys.presented(&presented.credential))
        .map(str::to_owned)
    else {
        return unproven(held, &not_routed(&held.name));
    };
    if ctx.dry_run {
        return would_rotate(held, ROTATING);
    }
    let Some(requests) = request_service(ctx, fillers).await else {
        return unproven(held, &not_routed(&held.name));
    };
    let gate = kept.gate();
    let media = super::gating::value(ctx, fillers).await;
    let serves = admitted
        .iter()
        .any(|route| gating::media_pins(&route.target).is_some());
    if serves && media.is_none() {
        return unproven(held, UNREAD_MEDIA);
    }
    let media = media.unwrap_or_default();
    let before = match super::gating::restated(ctx, fillers, &admitted, &media) {
        Ok(before) => before,
        Err(why) => return unproven(held, &why),
    };
    let new = match keys.beside(ctx, &old) {
        Ok(new) => new,
        Err(why) => return unproven(held, &why),
    };
    let after = presenting(&before, one.route, &new);
    let proven = match gate.admit(&after).await {
        Ok(()) => proved(requests.as_ref(), one).await,
        Err(failure) => Err(super::rotating::said(&failure)),
    };
    if let Err(why) = proven {
        let _ = gate.admit(&before).await;
        let _ = keys.only(ctx, &old);
        return unproven(held, &why);
    }
    Rotation::landed(
        &held.name,
        LANDED,
        vec![Propagation {
            consumer: consumer(one),
            reach: retired(keys.only(ctx, &new)),
        }],
    )
}

/// What taking the old key out came to: the gate presents the new one either way, and an
/// old key the adapter still admits is worth saying.
fn retired(taken_out: Result<(), String>) -> Reach {
    match taken_out {
        Ok(()) => Reach::Updated,
        Err(detail) => Reach::Failed { detail },
    }
}

/// The request service, asked as its owner.
async fn request_service(
    ctx: &Ctx,
    fillers: &Fillers,
) -> Option<std::sync::Arc<dyn lemonfiber_contract::capabilities::request::intake::Fills>> {
    let filler = crate::app::targets::request_service(fillers)?;
    crate::app::targets::requests_as_owner(ctx, filler).await
}

/// `routes`, the one answered under `route` presenting `key`.
fn presenting(routes: &[Gated], route: &str, key: &str) -> Vec<Gated> {
    routes
        .iter()
        .map(|one| {
            if one.path == route {
                Gated {
                    credential: key.to_owned(),
                    ..one.clone()
                }
            } else {
                one.clone()
            }
        })
        .collect()
}

/// Whether the request service's own test of its target at `one`'s route passes through
/// the gate.
async fn proved(requests: &dyn Requests, one: &Routed<'_>) -> Result<(), String> {
    let at = through_the_gate(one.route);
    let unreached = || {
        format!(
            "the request service could not reach {} through the gate under the new key, so \
             the old one was put back",
            one.curator.name
        )
    };
    let targets = requests.fulfilment_targets().await;
    let target = targets
        .iter()
        .flatten()
        .find(|target| target.at == at)
        .ok_or_else(unreached)?;
    requests
        .test_fulfilment_target(target.kind, &target.at, &target.key)
        .await
        .map_err(|_| unreached())
}

/// What is said where the key a line names is not presented by the gate yet.
fn not_routed(name: &str) -> String {
    format!("{name} is not presented by the request gate yet. Run lemonfiber seed.")
}

/// What the inventory calls the key on `one`.
fn name(one: &Routed<'_>) -> String {
    format!("Request gate key on {}", one.curator.name)
}

/// The one thing that authenticates with the key on `one`.
fn consumer(one: &Routed<'_>) -> String {
    format!(
        "the request gate, which asks {} for the request service's requests",
        one.curator.name
    )
}

#[cfg(test)]
mod tests;
