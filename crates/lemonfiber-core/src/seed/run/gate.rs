//! The request gate's routes: each upstream it answers for, and what it presents there.
//!
//! The gate holds the credentials the request service would otherwise hold: each
//! fulfilling \*arr's own key, read from where the \*arr wrote it, and a Jellyfin key
//! minted for the gate alone and filed under its name. They are handed over in one
//! owner-only file in the gate's configuration directory, never its environment, and
//! written again whenever what they should hold has moved — an \*arr that regenerated
//! its key reaches the gate on the next pass.
//!
//! **The file is what holds the Jellyfin key.** A key filed under the gate's name that
//! the file does not hold is one nothing holds, and is revoked, as the decline
//! service's are.

use std::path::Path;

use lemonfiber_sidecar::gate::{Credential, File, Kind, Upstream, Upstreams};

use super::minted;
use super::Ctx;
use crate::app::gating;
use crate::app::targets::ServiceAddr;
use crate::jellyfin::{Jellyfin, GATE_APP};
use crate::seed::{State, Wiring};

/// What the report calls this connection.
const CONNECTION: &str = "The request gate's routes";

/// Hold the gate to a route for each fulfilling \*arr and one for Jellyfin, under a key
/// of its own — or, where the stack no longer runs the gate, to no key at all.
pub(super) async fn seed_gate_routes(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Option<Wiring> {
    let jellyfin = super::identity::jellyfin_service(services)?;
    let gating = gating::service(services).is_some();
    // Minted with the administrator's session, which lemonfiber holds only on a server
    // it set up; a rehearsal before the first run finds none recorded yet.
    let Some(password) = super::identity::recorded_jellyfin_password(ctx) else {
        let minting = ctx.dry_run && super::identity::seerr_service(services).is_some();
        return (gating && minting).then(|| {
            settled(State::WouldWire {
                yours: None,
                ours: None,
            })
        });
    };
    let client = Jellyfin::authenticated(
        ctx.seams.http.clone(),
        &jellyfin.loopback,
        "jellyfin",
        crate::config::JELLYFIN_ADMIN_USER,
        password,
    );
    let filed = client.filed_as(GATE_APP).await;
    // A stack that does not run the gate asked for nothing here: a key list that could
    // not be read is left for the next run to retire from, not reported.
    if !gating {
        return minted::retired(ctx, &client, GATE_APP, CONNECTION, &filed.ok()?).await;
    }
    let filed = match filed {
        Ok(filed) => filed,
        Err(failure) => return Some(settled(crate::seed::unreached(&failure))),
    };
    let Some(project) = project else {
        return Some(settled(State::Skipped {
            reason: "there is no stack directory to hand the request gate its routes in".to_owned(),
        }));
    };
    let path = gating::path(project, File::Upstreams);
    let current = ctx
        .seams
        .filesystem
        .read(&path)
        .await
        .and_then(|text| Upstreams::read(&text).ok());
    let routes = arr_routes(ctx, services, project).await;
    let held = current
        .as_ref()
        .and_then(|upstreams| upstreams.route(&jellyfin.id))
        .map(|route| route.credential.reveal().to_owned())
        .filter(|key| filed.contains(key));
    let state = match held {
        Some(key) => {
            let wanted = Upstreams::of(with(&routes, to_jellyfin(&jellyfin, &key)));
            let others: Vec<&String> = filed.iter().filter(|one| **one != key).collect();
            kept(ctx, &client, current.as_ref(), &wanted, others, &path).await
        }
        None if ctx.dry_run => State::WouldWire {
            yours: Some(listed(current.as_ref())),
            ours: Some(named(&with(&routes, to_jellyfin(&jellyfin, "")))),
        },
        None => {
            let key = match minted::mint(&client, GATE_APP).await {
                Ok(key) => key,
                Err(state) => return Some(settled(state)),
            };
            let wanted = Upstreams::of(with(&routes, to_jellyfin(&jellyfin, &key)));
            if let Err(failure) = crate::config::store::write(&path, &wanted.written()) {
                let _ = client.revoke(&key).await;
                return Some(settled(State::Failed {
                    detail: format!(
                        "the routes could not be written to {}, so the new key was revoked \
                         again: {failure}",
                        path.display()
                    ),
                }));
            }
            minted::revoking(&client, &filed).await
        }
    };
    Some(settled(state))
}

/// The gate already holds a key Jellyfin lists: bring its routes up to what they should
/// be, and revoke whatever else is filed under its name.
async fn kept(
    ctx: &Ctx,
    client: &Jellyfin,
    current: Option<&Upstreams>,
    wanted: &Upstreams,
    others: Vec<&String>,
    path: &Path,
) -> State {
    let moved = current != Some(wanted);
    if !moved && others.is_empty() {
        return State::AlreadyWired;
    }
    // Said by what changes: the routes where they move, and otherwise the keys.
    if ctx.dry_run && moved {
        return State::WouldWire {
            yours: Some(listed(current)),
            ours: Some(listed(Some(wanted))),
        };
    }
    if ctx.dry_run {
        return State::WouldWire {
            yours: Some(minted::count(GATE_APP, others.len() + 1)),
            ours: Some(minted::count(GATE_APP, 1)),
        };
    }
    if moved {
        if let Err(failure) = crate::config::store::write(path, &wanted.written()) {
            return State::Failed {
                detail: format!(
                    "the routes could not be written to {}: {failure}",
                    path.display()
                ),
            };
        }
    }
    minted::revoking(client, others).await
}

/// A route for each \*arr the request service fulfils through, with the key it wrote for
/// itself. An \*arr that has written none yet has no route until it has.
async fn arr_routes(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: &Path,
) -> Vec<Upstream> {
    let mut routes = Vec::new();
    for arr in super::servarr_arrs(services, Some(project)) {
        let Some((television, (host, port))) = super::fulfilment::fetches(&arr.media_types)
            .zip(super::arrs::reached_at(services, &arr.target.id))
        else {
            continue;
        };
        let Some(key) = super::read_servarr_key(ctx, &arr.target.config).await else {
            continue;
        };
        routes.push(Upstream {
            route: arr.target.id.clone(),
            kind: if television {
                Kind::Sonarr
            } else {
                Kind::Radarr
            },
            address: format!("http://{host}:{port}"),
            credential: Credential::new(key),
        });
    }
    routes
}

/// The route to `jellyfin`, presenting `key`.
fn to_jellyfin(jellyfin: &ServiceAddr, key: &str) -> Upstream {
    Upstream {
        route: jellyfin.id.clone(),
        kind: Kind::Jellyfin,
        address: jellyfin.network_url.clone(),
        credential: Credential::new(key),
    }
}

/// `routes`, and the Jellyfin route after them.
fn with(routes: &[Upstream], jellyfin: Upstream) -> Vec<Upstream> {
    let mut all = routes.to_vec();
    all.push(jellyfin);
    all
}

/// What a set of routes answers for, said by their names and never their credentials.
fn named(routes: &[Upstream]) -> String {
    if routes.is_empty() {
        return "no routes".to_owned();
    }
    let names: Vec<&str> = routes.iter().map(|route| route.route.as_str()).collect();
    format!("routes to {}", names.join(", "))
}

/// What the routes file holds now, said the same way.
fn listed(upstreams: Option<&Upstreams>) -> String {
    named(upstreams.map_or(&[], |upstreams| upstreams.upstreams.as_slice()))
}

/// This connection, resting in `state`.
fn settled(state: State) -> Wiring {
    Wiring::settled(CONNECTION.to_owned(), state)
}
