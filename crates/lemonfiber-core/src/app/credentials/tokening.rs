//! The request gate's tokens, as the inventory lists and replaces them: one line per
//! route the gate answers.
//!
//! lemonfiber keeps no copy of a token. The request service holds each one raw and the
//! gate only its hash, so a line is read from the request service — its target at the
//! gate's route, or its media-server connection — and checked against the hashes the
//! gate accepts. Nothing here ever prints one.
//!
//! **A replacement keeps a working token at every moment.** A new token's hash is handed
//! to the gate beside the old, the request service is given the token and made to prove
//! it through the gate, and only then does the gate stop accepting the old one. Where the
//! proof fails the request service is put back on the old token and the new hash taken
//! out again.

use std::path::Path;

use lemonfiber_manifest::Service;
use lemonfiber_sidecar::gate::{File, Kind, Upstreams};

use super::rotating::{said, unproven, would_rotate};
use crate::app::gating;
use crate::app::Ctx;
use crate::credential::{fingerprint, Held, Origin, Propagation, Reach, Rotation, State};
use crate::ports::service::{MediaServerLink, RegisteredTarget, Requests};
use crate::seed::run::identity;
use crate::seed::run::tokens::{through_the_gate, Kept, Presented};

/// What each token is recorded as, before the route it is accepted on.
const SETTING: &str = "request-gate/tokens.json#";

/// Where each token is held.
const LOCATION: &str = "held only by the request service";

/// What a confirmed ask to print one is told.
pub(super) const UNPRINTED: &str =
    "lemonfiber keeps no copy of this token; only the request service holds it";

/// What is said where the request service could not be read.
const UNREAD: &str = "the request service could not be read, so this token was not checked.";

/// What a rehearsal says a replacement would take.
const ROTATING: &str = "a real run would mint a new token, hand the gate its hash beside the old \
                        one, give it to the request service, prove it with the request \
                        service's own test through the gate, and only then retire the old one. \
                        Nothing was minted here, and nothing was written.";

/// What a landed replacement says.
const LANDED: &str = "the request service took the new token, and the gate accepts it alone";

/// Why there is nothing to mint with.
const NO_RANDOMNESS: &str = "no randomness was available to generate a token";

/// Whether `held` is one of the gate's tokens.
pub(super) fn is_token(held: &Held) -> bool {
    held.setting.starts_with(SETTING)
}

/// One route the gate answers, as its routes file names it.
struct Route {
    /// Its path segment.
    id: String,
    /// What the household calls the service behind it.
    name: String,
    /// What it reaches.
    kind: Kind,
}

/// A token's line in the inventory, for each route the gate answers.
pub(super) async fn held(ctx: &Ctx, services: &[Service], project: Option<&Path>) -> Vec<Held> {
    let (Some(project), Some(base)) = (
        project.filter(|_| gating::service(services).is_some()),
        identity::seerr_service(services),
    ) else {
        return Vec::new();
    };
    let routes = routes(ctx, services, project).await;
    if routes.is_empty() {
        return Vec::new();
    }
    let seerr = crate::app::targets::seerr_as_owner(ctx, services, base).await;
    let targets = seerr.fulfilment_targets().await.ok();
    let link = seerr.media_server_link().await.ok();
    let kept = Kept::read(ctx, project).await;
    routes
        .iter()
        .map(|route| {
            let found = match route.kind {
                Kind::Jellyfin => link.as_ref().map(|link| linked(link, route)),
                Kind::Sonarr | Kind::Radarr => targets
                    .as_deref()
                    .map(|targets| targeted(targets, route).map(|target| target.key.clone())),
            };
            let found = match found {
                None => Found::Unread,
                Some(None) => Found::Unheld,
                Some(Some(token)) => Found::Token(token),
            };
            line(route, found, &kept)
        })
        .collect()
}

/// What the request service was found holding on a route.
enum Found {
    /// It could not be read.
    Unread,
    /// It holds no token there.
    Unheld,
    /// This token.
    Token(String),
}

/// The line for `route`, from what the request service was `found` holding there.
fn line(route: &Route, found: Found, kept: &Kept) -> Held {
    let (state, token, advisory) = match found {
        Found::Unread => (State::Stale, None, Some(UNREAD.to_owned())),
        Found::Unheld => (State::Absent, None, Some(not_handed(&name(route)))),
        Found::Token(token) if kept.accepts(&route.id, &token) => {
            (State::Active, Some(token), None)
        }
        Found::Token(token) => (
            State::Invalid,
            Some(token),
            Some(format!(
                "the gate does not accept the token the request service holds for {}. Run \
                 `lemonfiber seed`, which replaces it.",
                route.name
            )),
        ),
    };
    Held {
        name: name(route),
        setting: format!("{SETTING}{}", route.id),
        consumers: vec![consumer(route)],
        location: LOCATION.to_owned(),
        origin: Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state,
        fingerprint: token.as_deref().map(fingerprint),
        advisory,
    }
}

/// Replace the token one line names, keeping a working one at every moment.
pub(super) async fn rotate(
    ctx: &Ctx,
    held: &Held,
    services: &[Service],
    project: Option<&Path>,
) -> Rotation {
    let id = held.setting.trim_start_matches(SETTING);
    let (Some(project), Some(base)) = (project, identity::seerr_service(services)) else {
        return unproven(held, &not_handed(&held.name));
    };
    let Some(route) = routes(ctx, services, project)
        .await
        .into_iter()
        .find(|route| route.id == id)
    else {
        return unproven(held, &not_handed(&held.name));
    };
    if ctx.dry_run {
        return would_rotate(held, ROTATING);
    }
    let Some(token) = crate::secret::generate(ctx.seams.random.as_ref()) else {
        return unproven(held, NO_RANDOMNESS);
    };
    let seerr = crate::app::targets::seerr_as_owner(ctx, services, base).await;
    let kept = Kept::read(ctx, project).await;
    let presented = [Presented {
        route: route.id.clone(),
        token: token.clone(),
    }];
    let beside = match kept.beside(&presented) {
        Ok(beside) => beside,
        Err(reason) => return unproven(held, &kept.unwritten(&reason)),
    };
    let proven = match route.kind {
        Kind::Jellyfin => relinked(&seerr, &route, &token).await,
        Kind::Sonarr | Kind::Radarr => retargeted(&seerr, &route, &token).await,
    };
    if let Err(detail) = proven {
        let _ = kept.restore();
        return unproven(held, &detail);
    }
    if let Err(reason) = kept.only(&beside, &presented, &[true]) {
        return unproven(held, &kept.unwritten(&reason));
    }
    Rotation::landed(
        &held.name,
        LANDED,
        vec![Propagation {
            consumer: consumer(&route),
            reach: Reach::Updated,
        }],
    )
}

/// Give the request service's target at `route` the new token and have it prove it,
/// putting the old one back where the proof fails.
async fn retargeted(seerr: &dyn Requests, route: &Route, token: &str) -> Result<(), String> {
    let at = through_the_gate(&route.id);
    let targets = seerr
        .fulfilment_targets()
        .await
        .map_err(|failure| said(&failure))?;
    let Some(held) = targeted(&targets, route) else {
        return Err(not_handed(&name(route)));
    };
    seerr
        .move_fulfilment_target(held, &at, token)
        .await
        .map_err(|failure| said(&failure))?;
    let television = route.kind == Kind::Sonarr;
    if seerr
        .test_fulfilment_target(television, &at, token)
        .await
        .is_ok()
    {
        return Ok(());
    }
    let moved = RegisteredTarget {
        key: token.to_owned(),
        ..held.clone()
    };
    let _ = seerr.move_fulfilment_target(&moved, &at, &held.key).await;
    Err(unreached(route))
}

/// Give the request service's media-server connection the new token, which it proves
/// through the gate before it keeps it.
async fn relinked(seerr: &dyn Requests, route: &Route, token: &str) -> Result<(), String> {
    seerr
        .link_media_server(&through_the_gate(&route.id), token)
        .await
        .map_err(|_| unreached(route))
}

/// What is said where the token a line names is not handed over yet.
fn not_handed(name: &str) -> String {
    format!("{name} is not handed to the request service yet. Run `lemonfiber seed`.")
}

/// Why a new token was taken back out.
fn unreached(route: &Route) -> String {
    format!(
        "the request service could not reach {} through the gate with the new token, so the \
         old one was put back",
        route.name
    )
}

/// The target the request service holds at `route` through the gate.
fn targeted<'a>(targets: &'a [RegisteredTarget], route: &Route) -> Option<&'a RegisteredTarget> {
    let at = through_the_gate(&route.id);
    let television = route.kind == Kind::Sonarr;
    targets
        .iter()
        .find(|target| target.at == at && target.television == television)
}

/// The token the request service presents to Jellyfin, where it reaches it at `route`.
fn linked(link: &MediaServerLink, route: &Route) -> Option<String> {
    (link.at == through_the_gate(&route.id)).then(|| link.key.clone())
}

/// What the inventory calls the token for `route`.
fn name(route: &Route) -> String {
    format!("Request gate token for {}", route.name)
}

/// The one thing that presents the token for `route`.
fn consumer(route: &Route) -> String {
    format!(
        "the request service, which reaches {} through the gate",
        route.name
    )
}

/// The routes the gate's routes file under `project` names, each with the name of the
/// service it reaches.
async fn routes(ctx: &Ctx, services: &[Service], project: &Path) -> Vec<Route> {
    let path = gating::path(project, File::Upstreams);
    let Some(upstreams) = ctx
        .seams
        .filesystem
        .read(&path)
        .await
        .and_then(|text| Upstreams::read(&text).ok())
    else {
        return Vec::new();
    };
    upstreams
        .upstreams
        .into_iter()
        .map(|upstream| Route {
            name: services
                .iter()
                .find(|service| service.id == upstream.route)
                .map_or_else(|| upstream.route.clone(), |service| service.name.clone()),
            id: upstream.route,
            kind: upstream.kind,
        })
        .collect()
}

#[cfg(test)]
mod tests;
